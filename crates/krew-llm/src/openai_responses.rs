//! OpenAI Responses API (`POST /v1/responses`) implementation.

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
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
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    enable_web_search: bool,
    other_agent_role: OtherAgentRole,
    retry_config: RetryConfig,
}

impl OpenAiResponsesClient {
    /// Create a new OpenAI Responses API client.
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
            enable_thinking: config.enable_thinking,
            thinking_effort: config.thinking_effort,
            enable_web_search: config.enable_web_search,
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
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
    let mut result: Vec<serde_json::Value> = Vec::new();
    let mut pending: Vec<RoleContent> = Vec::new();

    for msg in messages {
        // Tool result messages: Responses API uses function_call_output.
        if msg.role == ChatRole::Tool {
            flush_pending_responses(&mut pending, &mut result);

            let mut obj = serde_json::json!({
                "type": "function_call_output",
                "output": msg.content,
            });
            if let Some(ref id) = msg.tool_call_id {
                obj["call_id"] = serde_json::json!(id);
            }
            result.push(obj);
            continue;
        }

        // Assistant messages with tool_calls: emit function_call items.
        if let (ChatRole::Assistant, Some(tcs)) = (&msg.role, &msg.tool_calls) {
            flush_pending_responses(&mut pending, &mut result);

            // Emit the assistant text message first (if any).
            if !msg.content.is_empty() {
                result.push(serde_json::json!({
                    "type": "message",
                    "role": "assistant",
                    "content": [{
                        "type": "output_text",
                        "text": msg.content,
                    }],
                    "status": "completed",
                }));
            }

            // Emit each tool call as a function_call item.
            for tc in tcs {
                result.push(serde_json::json!({
                    "type": "function_call",
                    "call_id": tc.id,
                    "name": tc.name,
                    "arguments": tc.arguments,
                    "status": "completed",
                }));
            }
            continue;
        }

        // Regular messages.
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

        let content = if is_other_agent {
            let name = msg.name.as_deref().unwrap_or("unknown");
            format!("[{name}] {}", msg.content)
        } else if msg.role == ChatRole::User {
            format!("[user] {}", msg.content)
        } else {
            msg.content.clone()
        };

        pending.push(RoleContent {
            role: role.to_string(),
            content,
        });
    }

    flush_pending_responses(&mut pending, &mut result);
    result
}

/// Merge and flush pending role-content items into the result vector.
fn flush_pending_responses(pending: &mut Vec<RoleContent>, result: &mut Vec<serde_json::Value>) {
    if pending.is_empty() {
        return;
    }
    let merged = merge_consecutive_same_role(std::mem::take(pending));
    for rc in merged {
        match rc.role.as_str() {
            "developer" => result.push(serde_json::json!({
                "type": "message",
                "role": "developer",
                "content": rc.content,
            })),
            "assistant" => result.push(serde_json::json!({
                "type": "message",
                "role": "assistant",
                "content": [{
                    "type": "output_text",
                    "text": rc.content,
                }],
                "status": "completed",
            })),
            _ => result.push(serde_json::json!({
                "type": "message",
                "role": "user",
                "content": rc.content,
            })),
        }
    }
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
                "strict": false,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// Pending events to drain (when multiple events must be emitted for a single SSE chunk).
type PendingQueue = std::collections::VecDeque<StreamEvent>;

/// Extract a display string for a web_search_call action.
/// Uses `query` for search actions and `url` for open_page actions.
fn extract_web_search_query(item: &serde_json::Value) -> Option<String> {
    let action = item.get("action")?;
    let action_type = action.get("type").and_then(|t| t.as_str()).unwrap_or("");
    match action_type {
        "open_page" => action
            .get("url")
            .and_then(|u| u.as_str())
            .map(|s| s.to_string()),
        _ => action
            .get("query")
            .and_then(|q| q.as_str())
            .map(|s| s.to_string()),
    }
}

/// Mutable state carried through the SSE `unfold` stream.
struct SseStreamState<S> {
    sse: S,
    pending: PendingQueue,
    done: bool,
    /// Whether `response.output_text.delta` events were received.
    has_streamed_text: bool,
    /// Whether `response.reasoning_summary_text.delta` events were received.
    has_streamed_thinking: bool,
    /// Whether `response.output_item.done` events were received
    /// (covers function_call and web_search_call).
    has_streamed_items: bool,
}

/// Parse OpenAI Responses SSE events into StreamEvents.
///
/// Uses `response.output_item.done` to extract complete function calls
/// (no incremental accumulation needed — the complete item is in one event).
///
/// When a proxy (e.g. litellm) falls back to fake streaming, content may
/// only appear in `response.completed`. The `has_streamed_*` flags track
/// what was already delivered incrementally so we can extract missing items
/// from `response.completed` without duplicating native OpenAI events.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;
    use std::collections::VecDeque;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    let state = SseStreamState {
        sse: sse_stream,
        pending: VecDeque::new(),
        done: false,
        has_streamed_text: false,
        has_streamed_thinking: false,
        has_streamed_items: false,
    };

    futures::stream::unfold(state, |mut st| async move {
        // Drain pending events first (multiple events from one SSE chunk).
        if let Some(event) = st.pending.pop_front() {
            return Some((event, st));
        }

        if st.done {
            return None;
        }

        loop {
            let next = st.sse.next().await;
            match next {
                Some(Ok(event)) => {
                    let event_type = event.event;
                    let data = event.data.trim().to_string();

                    if data.is_empty() || data == "[DONE]" {
                        continue;
                    }

                    // Some proxies (e.g. litellm) send all SSE events with
                    // the default "message" event type and put the real type
                    // inside the JSON `type` field. Detect and use that.
                    let effective_type = if event_type != "message" {
                        event_type
                    } else if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                        && let Some(t) = v.get("type").and_then(|t| t.as_str())
                    {
                        t.to_string()
                    } else {
                        event_type
                    };

                    match effective_type.as_str() {
                        "response.output_text.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                && let Some(delta) = v.get("delta").and_then(|d| d.as_str())
                                && !delta.is_empty()
                            {
                                st.has_streamed_text = true;
                                return Some((StreamEvent::TextDelta(delta.to_string()), st));
                            }
                            continue;
                        }

                        "response.reasoning_summary_text.delta" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                && let Some(delta) = v.get("delta").and_then(|d| d.as_str())
                                && !delta.is_empty()
                            {
                                st.has_streamed_thinking = true;
                                return Some((StreamEvent::ThinkingDelta(delta.to_string()), st));
                            }
                            continue;
                        }

                        "response.output_item.done" => {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                && let Some(item) = v.get("item")
                            {
                                let item_type =
                                    item.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                st.has_streamed_items = true;

                                // Server-side web search call completed.
                                if item_type == "web_search_call" {
                                    let query = extract_web_search_query(item);
                                    return Some((
                                        StreamEvent::ServerToolDone {
                                            name: "web_search".to_string(),
                                            query,
                                        },
                                        st,
                                    ));
                                }

                                // Complete function call item.
                                if item_type == "function_call" {
                                    let call_id = item
                                        .get("call_id")
                                        .and_then(|c| c.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let name = item
                                        .get("name")
                                        .and_then(|n| n.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let arguments = item
                                        .get("arguments")
                                        .and_then(|a| a.as_str())
                                        .unwrap_or("{}")
                                        .to_string();
                                    return Some((
                                        StreamEvent::ToolCall {
                                            id: call_id,
                                            name,
                                            arguments,
                                            thought_signature: None,
                                        },
                                        st,
                                    ));
                                }
                            }
                            continue;
                        }

                        "response.completed" => {
                            st.done = true;
                            let parsed = serde_json::from_str::<serde_json::Value>(&data).ok();
                            let resp = parsed.as_ref().and_then(|v| v.get("response"));

                            // Extract output items that were NOT already delivered
                            // via incremental streaming (proxy fake-stream fallback).
                            if let Some(output) = resp
                                .and_then(|r| r.get("output"))
                                .and_then(|o| o.as_array())
                            {
                                for item in output {
                                    let item_type =
                                        item.get("type").and_then(|t| t.as_str()).unwrap_or("");
                                    match item_type {
                                        "reasoning" if !st.has_streamed_thinking => {
                                            if let Some(summary) =
                                                item.get("summary").and_then(|s| s.as_array())
                                            {
                                                for part in summary {
                                                    if let Some(text) =
                                                        part.get("text").and_then(|t| t.as_str())
                                                        && !text.is_empty()
                                                    {
                                                        st.pending.push_back(
                                                            StreamEvent::ThinkingDelta(
                                                                text.to_string(),
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        "message" if !st.has_streamed_text => {
                                            if let Some(content) =
                                                item.get("content").and_then(|c| c.as_array())
                                            {
                                                for part in content {
                                                    if part.get("type").and_then(|t| t.as_str())
                                                        == Some("output_text")
                                                        && let Some(text) = part
                                                            .get("text")
                                                            .and_then(|t| t.as_str())
                                                        && !text.is_empty()
                                                    {
                                                        st.pending.push_back(
                                                            StreamEvent::TextDelta(
                                                                text.to_string(),
                                                            ),
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                        "function_call" if !st.has_streamed_items => {
                                            let call_id = item
                                                .get("call_id")
                                                .and_then(|c| c.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let name = item
                                                .get("name")
                                                .and_then(|n| n.as_str())
                                                .unwrap_or("")
                                                .to_string();
                                            let arguments = item
                                                .get("arguments")
                                                .and_then(|a| a.as_str())
                                                .unwrap_or("{}")
                                                .to_string();
                                            st.pending.push_back(StreamEvent::ToolCall {
                                                id: call_id,
                                                name,
                                                arguments,
                                                thought_signature: None,
                                            });
                                        }
                                        "web_search_call" if !st.has_streamed_items => {
                                            st.pending.push_back(StreamEvent::ServerToolStart {
                                                name: "web_search".to_string(),
                                            });
                                            let query = extract_web_search_query(item);
                                            st.pending.push_back(StreamEvent::ServerToolDone {
                                                name: "web_search".to_string(),
                                                query,
                                            });
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            let usage = if let Some(u) = resp.and_then(|r| r.get("usage")) {
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

                            // If we have pending events, queue Done and drain
                            // pending first.
                            if !st.pending.is_empty() {
                                st.pending.push_back(StreamEvent::Done(usage));
                                let first = st.pending.pop_front().unwrap();
                                return Some((first, st));
                            }

                            return Some((StreamEvent::Done(usage), st));
                        }

                        "response.failed" => {
                            st.done = true;
                            let msg = if let Ok(v) =
                                serde_json::from_str::<serde_json::Value>(&data)
                                && let Some(resp) = v.get("response")
                                && let Some(status) = resp.get("status_details")
                                && let Some(err) = status.get("error")
                                && let Some(message) = err.get("message").and_then(|m| m.as_str())
                            {
                                message.to_string()
                            } else {
                                "response failed".to_string()
                            };
                            return Some((StreamEvent::Error(msg), st));
                        }

                        "response.incomplete" => {
                            st.done = true;
                            return Some((
                                StreamEvent::Error("response incomplete".to_string()),
                                st,
                            ));
                        }

                        "response.output_item.added" => {
                            // Detect web search starting.
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                && let Some(item) = v.get("item")
                                && item.get("type").and_then(|t| t.as_str())
                                    == Some("web_search_call")
                            {
                                return Some((
                                    StreamEvent::ServerToolStart {
                                        name: "web_search".to_string(),
                                    },
                                    st,
                                ));
                            }
                            continue;
                        }

                        // Ignore all other events (response.queued, response.in_progress,
                        // response.content_part.added, response.output_text.done,
                        // response.function_call_arguments.*,
                        // response.reasoning_summary_text.done, etc.)
                        _ => continue,
                    }
                }
                Some(Err(e)) => {
                    st.done = true;
                    return Some((StreamEvent::Error(format!("SSE stream error: {e}")), st));
                }
                None => {
                    st.done = true;
                    return Some((StreamEvent::Error("stream interrupted".into()), st));
                }
            }
        }
    })
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
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
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
        if !tools.is_empty() || self.enable_web_search {
            let mut tool_list = convert_tools(tools);
            if self.enable_web_search {
                tool_list.push(serde_json::json!({ "type": "web_search" }));
            }
            body["tools"] = serde_json::json!(tool_list);
        }

        // Send request with retry.
        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "OpenAI Responses",
        };
        let auth = AuthMode::Bearer(&self.api_key);
        let response =
            common::send_with_retry(&req_config, &auth, None, &self.retry_config, on_retry).await?;

        // Guard: if the response is not SSE, read body as text and return an error.
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();
        if !content_type.contains("text/event-stream") {
            let body_text = response.text().await.unwrap_or_default();
            tracing::warn!(
                "OpenAI Responses: expected text/event-stream but got {content_type}: {body_text}"
            );
            return Err(LlmError::Api(format!(
                "unexpected content-type '{content_type}': {body_text}"
            )));
        }

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
    fn sse_output_item_done_function_call() {
        // response.output_item.done contains the complete function call item.
        let data = r#"{"item":{"type":"function_call","call_id":"call_123","name":"read_file","arguments":"{\"path\":\"src/main.rs\"}","status":"completed"}}"#;
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
        let arguments = item.get("arguments").and_then(|a| a.as_str()).unwrap();
        assert!(arguments.contains("main.rs"));
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
        let messages = vec![ChatMessage::text(ChatRole::User, "hello".to_string(), None)];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "message");
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "[user] hello");
    }

    #[test]
    fn convert_system_to_developer() {
        let messages = vec![ChatMessage::text(
            ChatRole::System,
            "you are helpful".to_string(),
            None,
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "developer");
    }

    #[test]
    fn convert_current_agent_assistant() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "my reply".to_string(),
            Some("agent1".to_string()),
        )];
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
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
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
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "assistant");
        assert_eq!(result[0]["content"][0]["text"], "[agent2] other reply");
    }

    #[test]
    fn convert_multiple_messages_order_preserved() {
        let messages = vec![
            ChatMessage::text(ChatRole::System, "sys".to_string(), None),
            ChatMessage::text(ChatRole::User, "hi".to_string(), None),
            ChatMessage::text(
                ChatRole::Assistant,
                "hello".to_string(),
                Some("agent1".to_string()),
            ),
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
        assert_eq!(result[0]["strict"], false);
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

    // ---- Web search injection tests ----

    #[test]
    fn web_search_tool_appended_to_tools_list() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read".to_string(),
            parameters: serde_json::json!({}),
        }];
        let mut tool_list = convert_tools(&tools);
        // Simulate web search injection.
        tool_list.push(serde_json::json!({ "type": "web_search" }));
        assert_eq!(tool_list.len(), 2);
        assert_eq!(tool_list[0]["type"], "function");
        assert_eq!(tool_list[1]["type"], "web_search");
    }

    #[test]
    fn web_search_only_no_function_tools() {
        let mut tool_list = convert_tools(&[]);
        tool_list.push(serde_json::json!({ "type": "web_search" }));
        assert_eq!(tool_list.len(), 1);
        assert_eq!(tool_list[0]["type"], "web_search");
    }
}
