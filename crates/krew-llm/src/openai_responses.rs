//! OpenAI Responses API (`POST /v1/responses`) implementation.
//!
//! Supports both standard OpenAI and Azure mode (when `azure_endpoint` is set).

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{ChatMessage, ChatRole, LlmClient, LlmError, StreamEvent, ToolDefinition, Usage};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::{SamplingConfig, ThinkingEffort};
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.openai.com";

/// OpenAI Responses API client.
pub struct OpenAiResponsesClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
    model: String,
    agent_name: String,
    /// Whether thinking/reasoning is enabled.
    enable_thinking: bool,
    /// Thinking effort level.
    thinking_effort: Option<ThinkingEffort>,
    /// How to present other agents' messages.
    other_agent_role: OtherAgentRole,
}

impl OpenAiResponsesClient {
    /// Create a new OpenAI Responses API client.
    ///
    /// `api_key` is the resolved API key value.
    /// `base_url` overrides the default `https://api.openai.com`.
    /// For Azure OpenAI, set `base_url` to
    /// `https://YOUR-RESOURCE.openai.azure.com/openai`.
    pub fn new(
        agent_name: String,
        model: String,
        api_key: String,
        base_url: Option<&str>,
        enable_thinking: bool,
        thinking_effort: Option<ThinkingEffort>,
        other_agent_role: OtherAgentRole,
    ) -> Self {
        let base_url = base_url
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .to_string();

        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key,
            model,
            agent_name,
            enable_thinking,
            thinking_effort,
            other_agent_role,
        }
    }
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Convert unified ChatMessages to OpenAI Responses API `input` format.
///
/// - System messages → `{type: "message", role: "developer", content: "..."}`
/// - User messages → `{type: "message", role: "user", content: "..."}`
/// - Current agent's assistant messages → `{type: "message", role: "assistant",
///   content: [{type: "output_text", text: "..."}], status: "completed"}`
/// - Other agents' assistant messages → role per `other_agent_role` with `[agent_name]` prefix
///
/// After role conversion, consecutive same-role messages are merged.
pub fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
) -> Vec<serde_json::Value> {
    // First pass: convert roles.
    let mut role_contents: Vec<(String, &ChatMessage)> = Vec::with_capacity(messages.len());

    for msg in messages {
        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        let role = match &msg.role {
            ChatRole::System => "developer",
            ChatRole::User => "user",
            ChatRole::Tool => "user",
            ChatRole::Assistant if is_other_agent => match other_agent_role {
                OtherAgentRole::User => "user",
                OtherAgentRole::Assistant => "assistant",
            },
            ChatRole::Assistant => "assistant",
        };

        role_contents.push((role.to_string(), msg));
    }

    // Separate non-mergeable (developer, assistant) from mergeable (user).
    // We need to merge consecutive same-role messages, but developer and
    // assistant messages have special formatting.
    // Strategy: build intermediate RoleContent list, merge, then format.
    let mut intermediate: Vec<(RoleContent, Option<&ChatMessage>)> =
        Vec::with_capacity(role_contents.len());

    for (role, msg) in &role_contents {
        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        let content = if is_other_agent {
            let name = msg.name.as_deref().unwrap_or("unknown");
            format!("[{name}] {}", msg.content)
        } else {
            msg.content.clone()
        };

        intermediate.push((
            RoleContent {
                role: role.clone(),
                content,
            },
            if role == "assistant" { Some(msg) } else { None },
        ));
    }

    // Merge consecutive same-role messages.
    let role_contents_only: Vec<RoleContent> =
        intermediate.iter().map(|(rc, _)| rc.clone()).collect();
    let merged = merge_consecutive_same_role(role_contents_only);

    // Format into JSON.
    merged
        .into_iter()
        .map(|rc| match rc.role.as_str() {
            "developer" => serde_json::json!({
                "type": "message",
                "role": "developer",
                "content": rc.content,
            }),
            "assistant" => serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": rc.content,
                }],
                "status": "completed",
            }),
            _ => serde_json::json!({
                "type": "message",
                "role": "user",
                "content": rc.content,
            }),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Sampling parameter mapping
// ---------------------------------------------------------------------------

/// Build sampling parameters for the Responses API.
///
/// Maps: temperature, top_p, max_tokens→max_output_tokens.
/// Ignores: frequency_penalty, presence_penalty, stop_sequences, top_k.
fn build_sampling_params(sampling: &SamplingConfig) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();

    if let Some(t) = sampling.temperature {
        params.insert("temperature".into(), serde_json::json!(t));
    }
    if let Some(p) = sampling.top_p {
        params.insert("top_p".into(), serde_json::json!(p));
    }
    if let Some(m) = sampling.max_tokens {
        params.insert("max_output_tokens".into(), serde_json::json!(m));
    }
    // frequency_penalty, presence_penalty, stop_sequences, top_k intentionally ignored.

    params
}

// ---------------------------------------------------------------------------
// Thinking/reasoning parameter injection
// ---------------------------------------------------------------------------

/// Build the reasoning parameter for the request body.
fn build_reasoning_params(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
) -> Option<serde_json::Value> {
    if !enable_thinking {
        return None;
    }

    let effort = match thinking_effort {
        Some(ThinkingEffort::Low) => "low",
        Some(ThinkingEffort::High) => "high",
        Some(ThinkingEffort::Medium) | None => "medium",
    };

    Some(serde_json::json!({
        "effort": effort,
        "summary": "auto",
    }))
}

// ---------------------------------------------------------------------------
// Tool definition conversion
// ---------------------------------------------------------------------------

/// Convert ToolDefinitions to Responses API format.
fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "type": "function",
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
                "strict": true,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// State machine for tracking function call context across SSE events.
#[derive(Default)]
struct FunctionCallState {
    /// Current function call ID (from response.output_item.added).
    call_id: String,
    /// Current function name (from response.output_item.added).
    name: String,
}

/// Parse OpenAI Responses SSE events into StreamEvents.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    let state = FunctionCallState::default();

    futures::stream::unfold(
        (sse_stream, state, false),
        |(mut sse_stream, mut fc_state, mut done)| async move {
            if done {
                return None;
            }

            loop {
                let next = sse_stream.next().await;
                match next {
                    Some(Ok(event)) => {
                        let event_type = event.event;
                        let data = event.data.trim().to_string();

                        if data.is_empty() {
                            continue;
                        }

                        match event_type.as_str() {
                            "response.output_text.delta" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(delta) = v.get("delta").and_then(|d| d.as_str())
                                    && !delta.is_empty()
                                {
                                    return Some((
                                        StreamEvent::TextDelta(delta.to_string()),
                                        (sse_stream, fc_state, done),
                                    ));
                                }
                                continue;
                            }

                            "response.reasoning_summary_text.delta" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(delta) = v.get("delta").and_then(|d| d.as_str())
                                    && !delta.is_empty()
                                {
                                    return Some((
                                        StreamEvent::ThinkingDelta(delta.to_string()),
                                        (sse_stream, fc_state, done),
                                    ));
                                }
                                continue;
                            }

                            "response.output_item.added" => {
                                // Cache function call metadata.
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(item) = v.get("item")
                                    && item.get("type").and_then(|t| t.as_str())
                                        == Some("function_call")
                                {
                                    fc_state.call_id = item
                                        .get("call_id")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    fc_state.name = item
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                }
                                continue;
                            }

                            "response.function_call_arguments.done" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) {
                                    let arguments = v
                                        .get("arguments")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("{}")
                                        .to_string();
                                    return Some((
                                        StreamEvent::ToolCall {
                                            id: fc_state.call_id.clone(),
                                            name: fc_state.name.clone(),
                                            arguments,
                                        },
                                        (sse_stream, fc_state, done),
                                    ));
                                }
                                continue;
                            }

                            "response.completed" => {
                                done = true;
                                let usage = if let Ok(v) =
                                    serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(resp) = v.get("response")
                                    && let Some(u) = resp.get("usage")
                                {
                                    Usage {
                                        prompt_tokens: u
                                            .get("input_tokens")
                                            .and_then(|t| t.as_u64())
                                            .unwrap_or(0)
                                            as u32,
                                        completion_tokens: u
                                            .get("output_tokens")
                                            .and_then(|t| t.as_u64())
                                            .unwrap_or(0)
                                            as u32,
                                        total_tokens: {
                                            let input = u
                                                .get("input_tokens")
                                                .and_then(|t| t.as_u64())
                                                .unwrap_or(0);
                                            let output = u
                                                .get("output_tokens")
                                                .and_then(|t| t.as_u64())
                                                .unwrap_or(0);
                                            (input + output) as u32
                                        },
                                    }
                                } else {
                                    Usage::default()
                                };
                                return Some((
                                    StreamEvent::Done(usage),
                                    (sse_stream, fc_state, done),
                                ));
                            }

                            "response.failed" => {
                                done = true;
                                let msg = if let Ok(v) =
                                    serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(resp) = v.get("response")
                                    && let Some(status) = resp.get("status_details")
                                    && let Some(err) = status.get("error")
                                    && let Some(message) =
                                        err.get("message").and_then(|m| m.as_str())
                                {
                                    message.to_string()
                                } else {
                                    "response failed".to_string()
                                };
                                return Some((
                                    StreamEvent::Error(msg),
                                    (sse_stream, fc_state, done),
                                ));
                            }

                            "response.incomplete" => {
                                done = true;
                                return Some((
                                    StreamEvent::Error("response incomplete".to_string()),
                                    (sse_stream, fc_state, done),
                                ));
                            }

                            // Ignore all other events (response.queued, response.in_progress,
                            // response.content_part.added, response.output_text.done,
                            // response.reasoning_summary_text.done, etc.)
                            _ => continue,
                        }
                    }
                    Some(Err(e)) => {
                        done = true;
                        return Some((
                            StreamEvent::Error(format!("SSE stream error: {e}")),
                            (sse_stream, fc_state, done),
                        ));
                    }
                    None => {
                        done = true;
                        return Some((
                            StreamEvent::Error("stream interrupted".into()),
                            (sse_stream, fc_state, done),
                        ));
                    }
                }
            }
        },
    )
}

// ---------------------------------------------------------------------------
// LlmClient implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl LlmClient for OpenAiResponsesClient {
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = format!("{}/v1/responses", self.base_url);

        // Build input array.
        let input = convert_messages(messages, &self.agent_name, &self.other_agent_role);

        // Build request body.
        let mut body = serde_json::json!({
            "model": self.model,
            "input": input,
            "stream": true,
        });

        // Merge sampling parameters.
        let sampling_params = build_sampling_params(sampling);
        for (k, v) in sampling_params {
            body[k] = v;
        }

        // Add reasoning if thinking is enabled.
        if let Some(reasoning) = build_reasoning_params(self.enable_thinking, self.thinking_effort)
        {
            body["reasoning"] = reasoning;
        }

        // Add tools if provided.
        if !tools.is_empty() {
            body["tools"] = serde_json::json!(convert_tools(tools));
        }

        // Send request with retry.
        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "OpenAI Responses",
        };
        let auth = AuthMode::Bearer(&self.api_key);
        let response = common::send_with_retry(&req_config, &auth, None).await?;

        // Convert to SSE event stream.
        let stream = build_event_stream(response);

        Ok(Box::pin(stream))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use krew_config::SamplingConfig;

    // ---- SSE parsing tests (3.8) ----

    #[test]
    fn sse_text_delta_event() {
        // Simulated: parse the event data directly
        let data = r#"{"delta":"hello"}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let delta = v.get("delta").and_then(|d| d.as_str()).unwrap();
        assert_eq!(delta, "hello");
    }

    #[test]
    fn sse_empty_delta_ignored() {
        let data = r#"{"delta":""}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let delta = v.get("delta").and_then(|d| d.as_str()).unwrap();
        assert!(delta.is_empty());
    }

    #[test]
    fn sse_function_call_done() {
        let data = r#"{"arguments":"{\"path\":\"src/main.rs\"}"}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let arguments = v.get("arguments").and_then(|a| a.as_str()).unwrap();
        assert!(arguments.contains("main.rs"));
    }

    #[test]
    fn sse_output_item_added_function_call() {
        let data = r#"{"item":{"type":"function_call","call_id":"call_123","name":"read_file"}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let item = v.get("item").unwrap();
        assert_eq!(
            item.get("type").and_then(|t| t.as_str()),
            Some("function_call")
        );
        assert_eq!(
            item.get("call_id").and_then(|c| c.as_str()),
            Some("call_123")
        );
        assert_eq!(item.get("name").and_then(|n| n.as_str()), Some("read_file"));
    }

    #[test]
    fn sse_response_completed_usage() {
        let data = r#"{"response":{"usage":{"input_tokens":100,"output_tokens":50}}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let u = v.get("response").unwrap().get("usage").unwrap();
        let input = u.get("input_tokens").unwrap().as_u64().unwrap() as u32;
        let output = u.get("output_tokens").unwrap().as_u64().unwrap() as u32;
        assert_eq!(input, 100);
        assert_eq!(output, 50);
    }

    #[test]
    fn sse_response_failed_error() {
        let data = r#"{"response":{"status_details":{"error":{"message":"rate limit exceeded"}}}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let msg = v
            .get("response")
            .unwrap()
            .get("status_details")
            .unwrap()
            .get("error")
            .unwrap()
            .get("message")
            .unwrap()
            .as_str()
            .unwrap();
        assert_eq!(msg, "rate limit exceeded");
    }

    // ---- Message conversion tests (3.9) ----

    #[test]
    fn convert_user_message() {
        let messages = vec![ChatMessage {
            role: ChatRole::User,
            content: "hello".to_string(),
            name: None,
        }];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "message");
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "hello");
    }

    #[test]
    fn convert_system_to_developer() {
        let messages = vec![ChatMessage {
            role: ChatRole::System,
            content: "you are helpful".to_string(),
            name: None,
        }];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "developer");
    }

    #[test]
    fn convert_current_agent_assistant() {
        let messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            content: "my reply".to_string(),
            name: Some("agent1".to_string()),
        }];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["status"], "completed");
        let content = result[0]["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "output_text");
        assert_eq!(content[0]["text"], "my reply");
    }

    #[test]
    fn convert_other_agent_to_user() {
        let messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            content: "other reply".to_string(),
            name: Some("agent2".to_string()),
        }];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "[agent2] other reply");
    }

    #[test]
    fn convert_other_agent_as_assistant() {
        let messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            content: "other reply".to_string(),
            name: Some("agent2".to_string()),
        }];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::Assistant);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"][0]["text"], "[agent2] other reply");
    }

    #[test]
    fn convert_multiple_messages_order_preserved() {
        let messages = vec![
            ChatMessage {
                role: ChatRole::System,
                content: "sys".to_string(),
                name: None,
            },
            ChatMessage {
                role: ChatRole::User,
                content: "hi".to_string(),
                name: None,
            },
            ChatMessage {
                role: ChatRole::Assistant,
                content: "hello".to_string(),
                name: Some("agent1".to_string()),
            },
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0]["role"], "developer");
        assert_eq!(result[1]["role"], "user");
        assert_eq!(result[2]["role"], "assistant");
    }

    #[test]
    fn convert_empty_messages() {
        let result = convert_messages(&[], "agent1", &OtherAgentRole::User);
        assert!(result.is_empty());
    }

    // ---- Sampling parameter tests (3.10) ----

    #[test]
    fn sampling_params_all_set() {
        let sampling = SamplingConfig {
            temperature: Some(0.7),
            top_p: Some(0.95),
            max_tokens: Some(4096),
            top_k: Some(40),                           // should be ignored
            frequency_penalty: Some(0.5),              // should be ignored
            presence_penalty: Some(0.3),               // should be ignored
            stop_sequences: Some(vec!["STOP".into()]), // should be ignored
        };
        let params = build_sampling_params(&sampling);
        assert_eq!(params["temperature"], 0.7);
        assert_eq!(params["top_p"], 0.95);
        assert_eq!(params["max_output_tokens"], 4096);
        assert!(!params.contains_key("top_k"));
        assert!(!params.contains_key("frequency_penalty"));
        assert!(!params.contains_key("presence_penalty"));
        assert!(!params.contains_key("stop_sequences"));
        assert!(!params.contains_key("stop"));
    }

    #[test]
    fn sampling_params_none() {
        let params = build_sampling_params(&SamplingConfig::default());
        assert!(params.is_empty());
    }

    #[test]
    fn sampling_max_tokens_maps_to_max_output_tokens() {
        let sampling = SamplingConfig {
            max_tokens: Some(8192),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling);
        assert!(params.contains_key("max_output_tokens"));
        assert!(!params.contains_key("max_tokens"));
        assert!(!params.contains_key("max_completion_tokens"));
    }

    // ---- URL construction tests ----

    #[test]
    fn default_url() {
        let base_url = DEFAULT_BASE_URL;
        let url = format!("{base_url}/v1/responses");
        assert_eq!(url, "https://api.openai.com/v1/responses");
    }

    // ---- Thinking/Reasoning parameter tests (3.12) ----

    #[test]
    fn reasoning_enabled_effort_high() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::High));
        let val = result.unwrap();
        assert_eq!(val["effort"], "high");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn reasoning_enabled_effort_low() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Low));
        let val = result.unwrap();
        assert_eq!(val["effort"], "low");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn reasoning_enabled_effort_none_defaults_to_medium() {
        let result = build_reasoning_params(true, None);
        let val = result.unwrap();
        assert_eq!(val["effort"], "medium");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn reasoning_disabled() {
        let result = build_reasoning_params(false, Some(ThinkingEffort::High));
        assert!(result.is_none());
    }

    // ---- Tool definition conversion tests (3.13) ----

    #[test]
    fn convert_single_tool() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }];
        let result = convert_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "function");
        assert_eq!(result[0]["name"], "read_file");
        assert_eq!(result[0]["description"], "Read a file");
        assert_eq!(result[0]["strict"], true);
        assert!(result[0]["parameters"].is_object());
    }

    #[test]
    fn convert_multiple_tools() {
        let tools = vec![
            ToolDefinition {
                name: "read_file".to_string(),
                description: "Read".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "write_file".to_string(),
                description: "Write".to_string(),
                parameters: serde_json::json!({}),
            },
        ];
        let result = convert_tools(&tools);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0]["name"], "read_file");
        assert_eq!(result[1]["name"], "write_file");
    }

    #[test]
    fn convert_empty_tools() {
        let result = convert_tools(&[]);
        assert!(result.is_empty());
    }
}
