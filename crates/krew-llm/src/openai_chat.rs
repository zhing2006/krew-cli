//! OpenAI Chat Completions API (`POST /v1/chat/completions`) implementation.

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::SamplingConfig;
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// OpenAI Chat Completions API client.
pub struct OpenAiChatClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    agent_name: String,
    other_agent_role: OtherAgentRole,
    retry_config: RetryConfig,
}

impl OpenAiChatClient {
    /// Create a new OpenAI Chat Completions client.
    pub fn new(config: LlmClientConfig) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();

        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: config.api_key,
            model: config.model,
            agent_name: config.agent_name,
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
        }
    }
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Convert unified ChatMessages to OpenAI Chat Completions message format.
///
/// - The current agent's own messages are always `role: assistant`.
/// - User messages are always `role: user`.
/// - Other agents' messages use `other_agent_role` for the role, with
///   content prefixed by `[agent_name]` for disambiguation.
///
/// Consecutive same-role messages are merged.
pub fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
) -> Vec<serde_json::Value> {
    let mut result: Vec<serde_json::Value> = Vec::new();

    // Collect plain messages for merging, flushing when we hit tool messages.
    let mut pending: Vec<RoleContent> = Vec::new();

    for msg in messages {
        // Tool result messages are emitted directly (not merged).
        if msg.role == ChatRole::Tool {
            // Flush any pending messages first.
            flush_pending(&mut pending, &mut result);

            let mut obj = serde_json::json!({
                "role": "tool",
                "content": msg.content,
            });
            if let Some(ref id) = msg.tool_call_id {
                obj["tool_call_id"] = serde_json::json!(id);
            }
            result.push(obj);
            continue;
        }

        // Assistant messages with tool_calls are emitted directly (not merged).
        if let (ChatRole::Assistant, Some(tcs)) = (&msg.role, &msg.tool_calls) {
            flush_pending(&mut pending, &mut result);

            let tool_calls: Vec<serde_json::Value> = tcs
                .iter()
                .map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.name,
                            "arguments": tc.arguments,
                        }
                    })
                })
                .collect();

            let mut obj = serde_json::json!({
                "role": "assistant",
                "tool_calls": tool_calls,
            });
            if !msg.content.is_empty() {
                obj["content"] = serde_json::json!(msg.content);
            }
            result.push(obj);
            continue;
        }

        // Regular messages: collect for merging.
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

        let content = if is_other_agent {
            let name = msg.name.as_deref().unwrap_or("unknown");
            format!("[{name}] {}", msg.content)
        } else {
            msg.content.clone()
        };

        pending.push(RoleContent {
            role: role.to_string(),
            content,
        });
    }

    flush_pending(&mut pending, &mut result);
    result
}

/// Merge and flush pending role-content items into the result vector.
fn flush_pending(pending: &mut Vec<RoleContent>, result: &mut Vec<serde_json::Value>) {
    if pending.is_empty() {
        return;
    }
    let merged = merge_consecutive_same_role(std::mem::take(pending));
    for rc in merged {
        result.push(serde_json::json!({
            "role": rc.role,
            "content": rc.content,
        }));
    }
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
        return Some(SseChunk {
            event: None,
            usage,
            tool_call_delta: None,
        });
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
            tool_call_delta: None,
        });
    }

    // Check for text content.
    if let Some(content) = delta.get("content").and_then(|c| c.as_str())
        && !content.is_empty()
    {
        return Some(SseChunk {
            event: Some(StreamEvent::TextDelta(content.to_string())),
            usage,
            tool_call_delta: None,
        });
    }

    // Check for tool calls (streamed incrementally — accumulate in build_event_stream).
    if let Some(tool_calls) = delta.get("tool_calls").and_then(|t| t.as_array())
        && let Some(tc) = tool_calls.first()
    {
        let index = tc.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as u32;
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
            event: None,
            usage,
            tool_call_delta: Some(ToolCallDelta {
                index,
                id,
                name,
                arguments,
            }),
        });
    }

    Some(SseChunk {
        event: None,
        usage,
        tool_call_delta: None,
    })
}

struct SseChunk {
    event: Option<StreamEvent>,
    usage: Option<Usage>,
    /// Partial tool call data to accumulate (OpenAI streams tool calls incrementally).
    tool_call_delta: Option<ToolCallDelta>,
}

/// A partial tool call chunk from OpenAI's incremental streaming.
struct ToolCallDelta {
    /// Tool call index (for parallel tool calls).
    index: u32,
    /// Call ID (only present in the first chunk for each tool call).
    id: String,
    /// Function name (only present in the first chunk).
    name: String,
    /// Partial arguments string to append.
    arguments: String,
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
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = format!("{}/v1/chat/completions", self.base_url);

        // Build messages array.
        let openai_messages = convert_messages(messages, &self.agent_name, &self.other_agent_role);

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
        let response =
            common::send_with_retry(&req_config, &auth, None, &self.retry_config, on_retry).await?;

        // Convert response into SSE event stream.
        let stream = build_event_stream(response);

        Ok(Box::pin(stream))
    }
}

/// Convert an HTTP response into a `Stream<Item = StreamEvent>`.
///
/// Tool calls are streamed incrementally by OpenAI (first chunk has id + name,
/// subsequent chunks carry partial arguments). This function accumulates them
/// and emits complete `StreamEvent::ToolCall` events only when `[DONE]` arrives.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;
    use std::collections::VecDeque;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    // We track usage across SSE chunks because OpenAI sends it in a
    // separate chunk just before [DONE].
    let usage_state = std::sync::Arc::new(tokio::sync::Mutex::new(Usage::default()));

    // Accumulated tool call deltas: Vec<(id, name, arguments)> indexed by tool call index.
    let tool_calls_accum: Vec<(String, String, String)> = Vec::new();

    // Queue for emitting multiple events at once (accumulated tool calls + Done).
    let pending: VecDeque<StreamEvent> = VecDeque::new();

    futures::stream::unfold(
        (sse_stream, usage_state, tool_calls_accum, pending, false),
        |(mut sse_stream, usage_state, mut tool_calls_accum, mut pending, mut done)| async move {
            // Drain pending events first (e.g., accumulated tool calls before Done).
            if let Some(event) = pending.pop_front() {
                return Some((
                    event,
                    (sse_stream, usage_state, tool_calls_accum, pending, done),
                ));
            }

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
                                // [DONE] marker — emit accumulated tool calls, then Done.
                                done = true;
                                for (id, name, args) in tool_calls_accum.drain(..) {
                                    pending.push_back(StreamEvent::ToolCall {
                                        id,
                                        name,
                                        arguments: args,
                                        thought_signature: None,
                                    });
                                }
                                let usage = usage_state.lock().await.clone();
                                pending.push_back(StreamEvent::Done(usage));

                                if let Some(event) = pending.pop_front() {
                                    return Some((
                                        event,
                                        (
                                            sse_stream,
                                            usage_state,
                                            tool_calls_accum,
                                            pending,
                                            done,
                                        ),
                                    ));
                                }
                                return None;
                            }
                            Some(chunk) => {
                                // Accumulate usage if present.
                                if let Some(u) = chunk.usage {
                                    let mut s = usage_state.lock().await;
                                    *s = u;
                                }

                                // Accumulate tool call deltas by index.
                                if let Some(delta) = chunk.tool_call_delta {
                                    let idx = delta.index as usize;
                                    if idx >= tool_calls_accum.len() {
                                        tool_calls_accum.resize(
                                            idx + 1,
                                            (String::new(), String::new(), String::new()),
                                        );
                                    }
                                    let entry = &mut tool_calls_accum[idx];
                                    if !delta.id.is_empty() {
                                        entry.0 = delta.id;
                                    }
                                    if !delta.name.is_empty() {
                                        entry.1 = delta.name;
                                    }
                                    entry.2.push_str(&delta.arguments);
                                    continue;
                                }

                                // Emit stream event if present.
                                if let Some(event) = chunk.event {
                                    return Some((
                                        event,
                                        (
                                            sse_stream,
                                            usage_state,
                                            tool_calls_accum,
                                            pending,
                                            done,
                                        ),
                                    ));
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
                            (sse_stream, usage_state, tool_calls_accum, pending, done),
                        ));
                    }
                    None => {
                        // Stream ended without [DONE] — interrupted.
                        done = true;
                        return Some((
                            StreamEvent::Error("stream interrupted".into()),
                            (sse_stream, usage_state, tool_calls_accum, pending, done),
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

    // ---- Message conversion tests ----

    #[test]
    fn convert_user_message() {
        let messages = vec![ChatMessage::text(ChatRole::User, "hello".to_string(), None)];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "hello");
    }

    #[test]
    fn convert_current_agent_assistant() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "my reply".to_string(),
            Some("agent1".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"], "my reply");
    }

    #[test]
    fn convert_other_agent_to_user() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "[agent2] other reply");
    }

    #[test]
    fn convert_other_agent_as_assistant() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::Assistant);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"], "[agent2] other reply");
    }

    #[test]
    fn convert_consecutive_same_role_merged() {
        let messages = vec![
            ChatMessage::text(
                ChatRole::Assistant,
                "reply A".to_string(),
                Some("agentA".to_string()),
            ),
            ChatMessage::text(
                ChatRole::Assistant,
                "reply B".to_string(),
                Some("agentB".to_string()),
            ),
        ];
        let result = convert_messages(&messages, "agentC", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "[agentA] reply A\n\n[agentB] reply B");
    }

    #[test]
    fn convert_empty_messages() {
        let result = convert_messages(&[], "agent1", &OtherAgentRole::User);
        assert!(result.is_empty());
    }

    // ---- SSE parsing tests ----

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
    fn sse_tool_call_delta() {
        // OpenAI streams tool calls incrementally — parse_sse_data returns
        // a ToolCallDelta, not a StreamEvent::ToolCall.
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_1","function":{"name":"read_file","arguments":"{\"path\":\"src/main.rs\"}"}}]}}]}"#;
        let chunk = parse_sse_data(data).unwrap();
        assert!(chunk.event.is_none(), "should not emit a StreamEvent");
        let delta = chunk.tool_call_delta.unwrap();
        assert_eq!(delta.index, 0);
        assert_eq!(delta.id, "call_1");
        assert_eq!(delta.name, "read_file");
        assert!(delta.arguments.contains("main.rs"));
    }

    #[test]
    fn sse_tool_call_delta_continuation() {
        // Continuation chunks only have partial arguments, no id or name.
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"more_args"}}]}}]}"#;
        let chunk = parse_sse_data(data).unwrap();
        assert!(chunk.event.is_none());
        let delta = chunk.tool_call_delta.unwrap();
        assert_eq!(delta.index, 0);
        assert!(delta.id.is_empty());
        assert!(delta.name.is_empty());
        assert_eq!(delta.arguments, "more_args");
    }
}
