//! OpenAI Chat Completions API (`POST /v1/chat/completions`) implementation.

use crate::common::{self, AuthMode, RequestConfig};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmError, OtherAgentRole, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::SamplingConfig;
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// OpenAI Chat Completions API client.
pub struct OpenAiChatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    /// Agent name for multi-agent message role attribution.
    agent_name: String,
    /// Whether to use the `name` field on messages for other agents.
    use_name_field: bool,
}

impl OpenAiChatClient {
    /// Create a new OpenAI Chat Completions client.
    ///
    /// `api_key_env` is the environment variable name holding the API key.
    /// `base_url` overrides the default `https://api.openai.com`.
    /// `use_name_field` controls whether other agents' messages use the `name`
    /// field or a `[name]` content prefix.
    pub fn new(
        agent_name: String,
        model: String,
        api_key_env: &str,
        base_url: Option<&str>,
        use_name_field: bool,
    ) -> Result<Self, LlmError> {
        let api_key = std::env::var(api_key_env).map_err(|_| {
            LlmError::Auth(format!(
                "environment variable {api_key_env} is not set or empty"
            ))
        })?;
        if api_key.is_empty() {
            return Err(LlmError::Auth(format!(
                "environment variable {api_key_env} is empty"
            )));
        }

        let base_url = base_url
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();

        Ok(Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            model,
            agent_name,
            use_name_field,
        })
    }
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Convert unified ChatMessages to OpenAI Chat Completions message format.
///
/// - The current agent's own messages are always `role: assistant` with no
///   `name` field.
/// - User messages are always `role: user` with no `name` field.
/// - Other agents' messages use `other_agent_role` for the role. When
///   `use_name_field` is true, the `name` field is set; otherwise the
///   content is prefixed with `[agent_name]`.
pub fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
    use_name_field: bool,
) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|msg| {
            let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
                && msg
                    .name
                    .as_ref()
                    .is_some_and(|name| name != self_agent_name);

            let role = match &msg.role {
                ChatRole::System => "system",
                ChatRole::User => "user",
                ChatRole::Tool => "tool",
                ChatRole::Assistant if is_other_agent => match other_agent_role {
                    OtherAgentRole::User => "user",
                    OtherAgentRole::Assistant => "assistant",
                },
                ChatRole::Assistant => "assistant",
            };

            let content = if is_other_agent && !use_name_field {
                // Prefix content with [agent_name] for disambiguation.
                let name = msg.name.as_deref().unwrap_or("unknown");
                format!("[{name}] {}", msg.content)
            } else {
                msg.content.clone()
            };

            let mut obj = serde_json::json!({
                "role": role,
                "content": content,
            });

            // Only set name field for other agents when explicitly enabled.
            if is_other_agent
                && use_name_field
                && let Some(name) = &msg.name
            {
                obj["name"] = serde_json::Value::String(name.clone());
            }

            obj
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sampling parameter mapping
// ---------------------------------------------------------------------------

/// Build the sampling fields for the OpenAI request body.
fn build_sampling_params(sampling: &SamplingConfig) -> serde_json::Value {
    let mut params = serde_json::Map::new();

    if let Some(t) = sampling.temperature {
        params.insert("temperature".into(), serde_json::json!(t));
    }
    if let Some(p) = sampling.top_p {
        params.insert("top_p".into(), serde_json::json!(p));
    }
    // top_k is not supported by OpenAI — intentionally ignored.
    if let Some(m) = sampling.max_tokens {
        params.insert("max_completion_tokens".into(), serde_json::json!(m));
    }
    if let Some(fp) = sampling.frequency_penalty {
        params.insert("frequency_penalty".into(), serde_json::json!(fp));
    }
    if let Some(pp) = sampling.presence_penalty {
        params.insert("presence_penalty".into(), serde_json::json!(pp));
    }
    if let Some(ref stops) = sampling.stop_sequences {
        params.insert("stop".into(), serde_json::json!(stops));
    }

    serde_json::Value::Object(params)
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// Parse a single SSE data line from OpenAI's Chat Completions API.
///
/// Returns `None` for the terminal `[DONE]` marker.
fn parse_sse_data(data: &str) -> Option<SseChunk> {
    if data == "[DONE]" {
        return None;
    }

    let v: serde_json::Value = serde_json::from_str(data).ok()?;
    let choices = v.get("choices")?.as_array()?;

    // Extract usage from chunks that carry it (stream_options.include_usage).
    let usage = v.get("usage").and_then(|u| {
        Some(Usage {
            prompt_tokens: u.get("prompt_tokens")?.as_u64()? as u32,
            completion_tokens: u.get("completion_tokens")?.as_u64()? as u32,
            total_tokens: u.get("total_tokens")?.as_u64()? as u32,
        })
    });

    if choices.is_empty() {
        // Usage-only chunk (the last chunk before [DONE]).
        return Some(SseChunk { event: None, usage });
    }

    let choice = &choices[0];
    let delta = choice.get("delta")?;

    // Check for reasoning/thinking content (e.g. DeepSeek, Doubao).
    if let Some(reasoning) = delta.get("reasoning_content").and_then(|c| c.as_str())
        && !reasoning.is_empty()
    {
        return Some(SseChunk {
            event: Some(StreamEvent::ThinkingDelta(reasoning.to_string())),
            usage,
        });
    }

    // Check for text content.
    if let Some(content) = delta.get("content").and_then(|c| c.as_str())
        && !content.is_empty()
    {
        return Some(SseChunk {
            event: Some(StreamEvent::TextDelta(content.to_string())),
            usage,
        });
    }

    // Check for tool calls.
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array())
        && let Some(tc) = tool_calls.first()
    {
        let id = tc
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let function = tc.get("function");
        let name = function
            .and_then(|f| f.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let arguments = function
            .and_then(|f| f.get("arguments"))
            .and_then(|a| a.as_str())
            .unwrap_or("")
            .to_string();

        return Some(SseChunk {
            event: Some(StreamEvent::ToolCall {
                id,
                name,
                arguments,
            }),
            usage,
        });
    }

    Some(SseChunk { event: None, usage })
}

struct SseChunk {
    event: Option<StreamEvent>,
    usage: Option<Usage>,
}

// ---------------------------------------------------------------------------
// LlmClient implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl LlmClient for OpenAiChatClient {
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Build messages array.
        let openai_messages = convert_messages(
            messages,
            &self.agent_name,
            &OtherAgentRole::User,
            self.use_name_field,
        );

        // Build request body.
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": openai_messages,
            "stream": true,
            "stream_options": { "include_usage": true },
        });

        // Merge sampling parameters.
        let sampling_params = build_sampling_params(sampling);
        if let serde_json::Value::Object(map) = sampling_params {
            for (k, v) in map {
                body[k] = v;
            }
        }

        // Add tools if provided.
        if !tools.is_empty() {
            let tool_defs: Vec<serde_json::Value> = tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tool_defs);
        }

        // Attempt request with retry logic.
        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "OpenAI",
        };
        let auth = AuthMode::Bearer(&self.api_key);
        let response = common::send_with_retry(&req_config, &auth, None).await?;

        // Convert response into SSE event stream.
        let stream = build_event_stream(response);

        Ok(Box::pin(stream))
    }
}

/// Convert an HTTP response into a `Stream<Item = StreamEvent>`.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    // We track usage across SSE chunks because OpenAI sends it in a
    // separate chunk just before [DONE].
    let state = std::sync::Arc::new(tokio::sync::Mutex::new(Usage::default()));

    futures::stream::unfold(
        (sse_stream, state, false),
        |(mut sse_stream, state, mut done)| async move {
            if done {
                return None;
            }

            loop {
                let next = sse_stream.next().await;
                match next {
                    Some(Ok(event)) => {
                        let data = event.data.trim().to_string();
                        if data.is_empty() {
                            continue;
                        }

                        match parse_sse_data(&data) {
                            None => {
                                // [DONE] marker — emit Done with accumulated usage.
                                done = true;
                                let usage = state.lock().await.clone();
                                return Some((StreamEvent::Done(usage), (sse_stream, state, done)));
                            }
                            Some(chunk) => {
                                // Accumulate usage if present.
                                if let Some(u) = chunk.usage {
                                    let mut s = state.lock().await;
                                    *s = u;
                                }

                                // Emit stream event if present.
                                if let Some(event) = chunk.event {
                                    return Some((event, (sse_stream, state, done)));
                                }
                                // No event (e.g. usage-only chunk) — continue.
                                continue;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        // SSE parsing error — emit error and stop.
                        done = true;
                        return Some((
                            StreamEvent::Error(format!("SSE stream error: {e}")),
                            (sse_stream, state, done),
                        ));
                    }
                    None => {
                        // Stream ended without [DONE] — interrupted.
                        done = true;
                        return Some((
                            StreamEvent::Error("stream interrupted".into()),
                            (sse_stream, state, done),
                        ));
                    }
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use krew_config::SamplingConfig;

    #[test]
    fn sampling_params_all_set() {
        let sampling = SamplingConfig {
            temperature: Some(0.7),
            top_p: Some(0.95),
            top_k: Some(40),
            max_tokens: Some(4096),
            frequency_penalty: Some(0.5),
            presence_penalty: Some(0.3),
            stop_sequences: Some(vec!["STOP".into()]),
        };

        let params = build_sampling_params(&sampling);
        assert_eq!(params["temperature"], 0.7);
        assert_eq!(params["top_p"], 0.95);
        assert_eq!(params["max_completion_tokens"], 4096);
        assert_eq!(params["frequency_penalty"], 0.5);
        assert_eq!(params["presence_penalty"], 0.3);
        assert_eq!(params["stop"], serde_json::json!(["STOP"]));
        assert!(params.get("top_k").is_none());
    }

    #[test]
    fn sampling_params_none() {
        let params = build_sampling_params(&SamplingConfig::default());
        assert_eq!(params, serde_json::json!({}));
    }

    #[test]
    fn sse_text_delta() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"}}]}"#;
        let chunk = parse_sse_data(data).unwrap();
        match chunk.event.unwrap() {
            StreamEvent::TextDelta(text) => assert_eq!(text, "Hello"),
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn sse_done_marker() {
        assert!(parse_sse_data("[DONE]").is_none());
    }

    #[test]
    fn sse_usage_chunk() {
        let data = r#"{"choices":[],"usage":{"prompt_tokens":100,"completion_tokens":50,"total_tokens":150}}"#;
        let chunk = parse_sse_data(data).unwrap();
        assert!(chunk.event.is_none());
        let usage = chunk.usage.unwrap();
        assert_eq!(usage.prompt_tokens, 100);
        assert_eq!(usage.completion_tokens, 50);
        assert_eq!(usage.total_tokens, 150);
    }

    #[test]
    fn sse_reasoning_content() {
        let data = r#"{"choices":[{"delta":{"reasoning_content":"Let me think..."}}]}"#;
        let chunk = parse_sse_data(data).unwrap();
        match chunk.event.unwrap() {
            StreamEvent::ThinkingDelta(text) => assert_eq!(text, "Let me think..."),
            other => panic!("expected ThinkingDelta, got {other:?}"),
        }
    }

    #[test]
    fn sse_tool_call() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}}]}}]}"#;
        let chunk = parse_sse_data(data).unwrap();
        match chunk.event.unwrap() {
            StreamEvent::ToolCall {
                id,
                name,
                arguments,
            } => {
                assert_eq!(id, "call_1");
                assert_eq!(name, "read_file");
                assert!(arguments.contains("main.rs"));
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }
}
