//! OpenAI Responses API (`POST /v1/responses`) implementation.

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::{ReasoningContext, ReasoningMode, SamplingConfig, ThinkingEffort};
use krew_config::{is_gpt_5_6_model, is_official_openai_base_url};
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
    reasoning_mode: Option<ReasoningMode>,
    reasoning_context: Option<ReasoningContext>,
    enable_web_search: bool,
    other_agent_role: OtherAgentRole,
    retry_config: RetryConfig,
    extra_headers: Vec<(String, String)>,
}

impl OpenAiResponsesClient {
    /// Create a new OpenAI Responses API client.
    pub fn new(config: LlmClientConfig) -> Self {
        let base_url = config
            .base_url
            .as_deref()
            .unwrap_or(DEFAULT_BASE_URL)
            .trim_end_matches('/')
            .trim_end_matches("/v1")
            .to_string();

        Self {
            http: reqwest::Client::new(),
            base_url,
            api_key: config.api_key,
            model: config.model,
            agent_name: config.agent_name,
            enable_thinking: config.enable_thinking,
            thinking_effort: config.thinking_effort,
            reasoning_mode: config.reasoning_mode,
            reasoning_context: config.reasoning_context,
            enable_web_search: config.enable_web_search,
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
            extra_headers: config.extra_headers,
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
    convert_messages_with_raw_replay(messages, self_agent_name, other_agent_role, false)
}

fn convert_messages_with_raw_replay(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
    replay_raw_output_items: bool,
) -> Vec<serde_json::Value> {
    let mut result: Vec<serde_json::Value> = Vec::new();
    let mut pending: Vec<RoleContent> = Vec::new();

    for msg in messages {
        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        // Replay the current OpenAI agent's output items verbatim. This keeps
        // encrypted reasoning content and its ordering intact across turns.
        if replay_raw_output_items
            && msg.role == ChatRole::Assistant
            && !is_other_agent
            && !msg.raw_content_blocks.is_empty()
        {
            flush_pending_responses(&mut pending, &mut result);
            result.extend(msg.raw_content_blocks.iter().cloned());
            continue;
        }

        // Tool result messages: Responses API uses function_call_output.
        if msg.role == ChatRole::Tool {
            flush_pending_responses(&mut pending, &mut result);

            let output = if msg.images.is_empty() {
                // Plain text output.
                serde_json::json!(msg.content)
            } else {
                // Multimodal: input_image blocks + input_text block.
                let mut blocks: Vec<serde_json::Value> = msg
                    .images
                    .iter()
                    .map(|img| {
                        let data_url = format!(
                            "data:{};base64,{}",
                            img.media_type,
                            common::encode_base64(&img.data)
                        );
                        serde_json::json!({
                            "type": "input_image",
                            "image_url": data_url,
                            "detail": "auto",
                        })
                    })
                    .collect();
                blocks.push(serde_json::json!({
                    "type": "input_text",
                    "text": msg.content,
                }));
                serde_json::json!(blocks)
            };

            let mut obj = serde_json::json!({
                "type": "function_call_output",
                "output": output,
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

/// Check if a model supports xhigh reasoning effort (whitelist).
///
/// Matches exact model name or model name with date suffix (e.g. "gpt-5.4-20260101").
fn supports_xhigh(model: &str) -> bool {
    if is_gpt_5_6_model(model) {
        return true;
    }

    const XHIGH_MODELS: &[&str] = &[
        "gpt-5.5",
        "gpt-5.5-pro",
        "gpt-5.4",
        "gpt-5.4-pro",
        "gpt-5.3-codex",
        "gpt-5.2",
    ];
    XHIGH_MODELS.iter().any(|m| {
        model == *m
            || (model.starts_with(m)
                && model.as_bytes().get(m.len()) == Some(&b'-')
                && model
                    .as_bytes()
                    .get(m.len() + 1)
                    .is_some_and(|c| c.is_ascii_digit()))
    })
}

/// Build the reasoning parameter for the request body.
fn build_reasoning_params_with_options(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    reasoning_mode: Option<ReasoningMode>,
    reasoning_context: Option<ReasoningContext>,
    model: &str,
) -> Option<serde_json::Value> {
    let effort = match thinking_effort {
        Some(ThinkingEffort::None) => Some("none"),
        Some(ThinkingEffort::Low) => Some("low"),
        Some(ThinkingEffort::High) => Some("high"),
        Some(ThinkingEffort::Xhigh) => Some(if supports_xhigh(model) {
            "xhigh"
        } else {
            "high"
        }),
        Some(ThinkingEffort::Max) => Some(if is_gpt_5_6_model(model) {
            "max"
        } else if supports_xhigh(model) {
            "xhigh"
        } else {
            "high"
        }),
        Some(ThinkingEffort::Medium) => Some("medium"),
        None if enable_thinking => Some("medium"),
        None => None,
    };

    let mut reasoning = serde_json::Map::new();
    if let Some(effort) = effort {
        reasoning.insert("effort".to_string(), serde_json::json!(effort));
    }
    if enable_thinking && thinking_effort != Some(ThinkingEffort::None) {
        reasoning.insert("summary".to_string(), serde_json::json!("auto"));
    }
    if reasoning_mode == Some(ReasoningMode::Pro) {
        reasoning.insert("mode".to_string(), serde_json::json!("pro"));
    }
    match reasoning_context {
        Some(ReasoningContext::CurrentTurn) => {
            reasoning.insert("context".to_string(), serde_json::json!("current_turn"));
        }
        Some(ReasoningContext::AllTurns) => {
            reasoning.insert("context".to_string(), serde_json::json!("all_turns"));
        }
        Some(ReasoningContext::Auto) | None => {}
    }

    if reasoning.is_empty() {
        None
    } else {
        Some(serde_json::Value::Object(reasoning))
    }
}

#[cfg(test)]
fn build_reasoning_params(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> Option<serde_json::Value> {
    build_reasoning_params_with_options(enable_thinking, thinking_effort, None, None, model)
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
    /// Stable keys for output items already emitted as raw replay data.
    seen_output_items: std::collections::HashSet<String>,
}

fn output_item_key(item: &serde_json::Value) -> String {
    if let Some(id) = item.get("id").and_then(|value| value.as_str()) {
        return format!("id:{id}");
    }
    if let Some(call_id) = item.get("call_id").and_then(|value| value.as_str()) {
        return format!("call_id:{call_id}");
    }
    serde_json::to_string(item).unwrap_or_default()
}

fn enqueue_output_item_events(
    pending: &mut PendingQueue,
    seen_output_items: &mut std::collections::HashSet<String>,
    item: &serde_json::Value,
    include_fallback_content: bool,
    capture_raw_output_item: bool,
) {
    if !seen_output_items.insert(output_item_key(item)) {
        return;
    }

    if capture_raw_output_item {
        pending.push_back(StreamEvent::RawContentBlock(item.clone()));
    }

    let item_type = item
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("");
    match item_type {
        "reasoning" if include_fallback_content => {
            if let Some(summary) = item.get("summary").and_then(|value| value.as_array()) {
                for part in summary {
                    if let Some(text) = part.get("text").and_then(|value| value.as_str())
                        && !text.is_empty()
                    {
                        pending.push_back(StreamEvent::ThinkingDelta(text.to_string()));
                    }
                }
            }
        }
        "message" if include_fallback_content => {
            if let Some(content) = item.get("content").and_then(|value| value.as_array()) {
                for part in content {
                    if part.get("type").and_then(|value| value.as_str()) == Some("output_text")
                        && let Some(text) = part.get("text").and_then(|value| value.as_str())
                        && !text.is_empty()
                    {
                        pending.push_back(StreamEvent::TextDelta(text.to_string()));
                    }
                }
            }
        }
        "function_call" => {
            pending.push_back(StreamEvent::ToolCall {
                id: item
                    .get("call_id")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                name: item
                    .get("name")
                    .and_then(|value| value.as_str())
                    .unwrap_or("")
                    .to_string(),
                arguments: item
                    .get("arguments")
                    .and_then(|value| value.as_str())
                    .unwrap_or("{}")
                    .to_string(),
                thought_signature: None,
            });
        }
        "web_search_call" => {
            if include_fallback_content {
                pending.push_back(StreamEvent::ServerToolStart {
                    name: "web_search".to_string(),
                });
            }
            pending.push_back(StreamEvent::ServerToolDone {
                name: "web_search".to_string(),
                query: extract_web_search_query(item),
            });
        }
        _ => {}
    }
}

fn enable_encrypted_reasoning_replay(
    body: &mut serde_json::Value,
    base_url: &str,
    model: &str,
) -> bool {
    if !supports_encrypted_reasoning_replay(base_url, model) {
        return false;
    }
    body["store"] = serde_json::json!(false);
    body["include"] = serde_json::json!(["reasoning.encrypted_content"]);
    true
}

fn supports_encrypted_reasoning_replay(base_url: &str, model: &str) -> bool {
    is_official_openai_base_url(Some(base_url)) && is_gpt_5_6_model(model)
}

/// Parse OpenAI Responses SSE events into StreamEvents.
///
/// Uses `response.output_item.done` to extract complete function calls
/// (no incremental accumulation needed — the complete item is in one event).
///
/// When a proxy (e.g. litellm) falls back to fake streaming, content may
/// only appear in `response.completed`. The stream state tracks
/// what was already delivered incrementally so we can extract missing items
/// from `response.completed` without duplicating native OpenAI events.
fn build_event_stream(
    response: reqwest::Response,
    capture_raw_output_items: bool,
) -> impl Stream<Item = StreamEvent> + Send {
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
        seen_output_items: std::collections::HashSet::new(),
    };

    futures::stream::unfold(state, move |mut st| async move {
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
                                let include_fallback_content = match item
                                    .get("type")
                                    .and_then(|value| value.as_str())
                                    .unwrap_or("")
                                {
                                    "reasoning" => !st.has_streamed_thinking,
                                    "message" => !st.has_streamed_text,
                                    _ => false,
                                };
                                enqueue_output_item_events(
                                    &mut st.pending,
                                    &mut st.seen_output_items,
                                    item,
                                    include_fallback_content,
                                    capture_raw_output_items,
                                );
                                if let Some(event) = st.pending.pop_front() {
                                    return Some((event, st));
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
                                    let item_type = item
                                        .get("type")
                                        .and_then(|value| value.as_str())
                                        .unwrap_or("");
                                    let include_fallback_content = match item_type {
                                        "reasoning" => !st.has_streamed_thinking,
                                        "message" => !st.has_streamed_text,
                                        _ => true,
                                    };
                                    enqueue_output_item_events(
                                        &mut st.pending,
                                        &mut st.seen_output_items,
                                        item,
                                        include_fallback_content,
                                        capture_raw_output_items,
                                    );
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

        let is_official_gpt_5_6 = supports_encrypted_reasoning_replay(&self.base_url, &self.model);

        // Replay encrypted output items only for official GPT-5.6 requests.
        let input = convert_messages_with_raw_replay(
            messages,
            &self.agent_name,
            &self.other_agent_role,
            is_official_gpt_5_6,
        );

        // Build request body.
        let mut body = serde_json::json!({
            "model": self.model,
            "input": input,
            "stream": true,
        });

        enable_encrypted_reasoning_replay(&mut body, &self.base_url, &self.model);

        // Merge sampling parameters.
        let sampling_params = build_sampling_params(sampling);
        for (k, v) in sampling_params {
            body[k] = v;
        }

        // GPT-5.6 mode and context are only sent to the official Responses API.
        let reasoning_mode = is_official_gpt_5_6.then_some(self.reasoning_mode).flatten();
        let reasoning_context = is_official_gpt_5_6
            .then_some(self.reasoning_context)
            .flatten();
        if let Some(reasoning) = build_reasoning_params_with_options(
            self.enable_thinking,
            self.thinking_effort,
            reasoning_mode,
            reasoning_context,
            &self.model,
        ) {
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
        let response = common::send_with_retry(
            &req_config,
            &auth,
            if self.extra_headers.is_empty() {
                None
            } else {
                Some(&self.extra_headers)
            },
            &self.retry_config,
            on_retry,
        )
        .await?;

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
        let stream = build_event_stream(response, is_official_gpt_5_6);

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
    fn convert_replays_current_agent_output_items_verbatim() {
        let mut message = ChatMessage::text(
            ChatRole::Assistant,
            "flattened text",
            Some("agent1".to_string()),
        );
        let reasoning = serde_json::json!({
            "id": "rs_123",
            "type": "reasoning",
            "encrypted_content": "encrypted-payload",
            "summary": [],
        });
        let output_message = serde_json::json!({
            "id": "msg_123",
            "type": "message",
            "role": "assistant",
            "status": "completed",
            "content": [{"type": "output_text", "text": "flattened text"}],
        });
        message.raw_content_blocks = vec![reasoning.clone(), output_message.clone()];

        let result =
            convert_messages_with_raw_replay(&[message], "agent1", &OtherAgentRole::User, true);
        assert_eq!(result, vec![reasoning, output_message]);
    }

    #[test]
    fn convert_ignores_raw_output_items_without_replay_capability() {
        let mut message = ChatMessage::text(
            ChatRole::Assistant,
            "flattened text",
            Some("agent1".to_string()),
        );
        message.raw_content_blocks = vec![serde_json::json!({
            "id": "rs_123",
            "type": "reasoning",
            "encrypted_content": "encrypted-payload",
        })];

        let result = convert_messages(&[message], "agent1", &OtherAgentRole::User);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["type"], "message");
        assert_eq!(result[0]["content"][0]["text"], "flattened text");
    }

    #[test]
    fn convert_does_not_replay_other_agents_output_items() {
        let mut message = ChatMessage::text(
            ChatRole::Assistant,
            "other reply",
            Some("agent2".to_string()),
        );
        message.raw_content_blocks = vec![serde_json::json!({
            "id": "rs_private",
            "type": "reasoning",
            "encrypted_content": "encrypted-payload",
        })];

        let result =
            convert_messages_with_raw_replay(&[message], "agent1", &OtherAgentRole::User, true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["role"], "user");
        assert_eq!(result[0]["content"], "[agent2] other reply");
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

    #[test]
    fn gpt_5_6_enables_stateless_encrypted_reasoning_replay() {
        let mut body = serde_json::json!({});
        assert!(enable_encrypted_reasoning_replay(
            &mut body,
            "https://api.openai.com",
            "openai/gpt-5.6-terra",
        ));
        assert_eq!(body["store"], false);
        assert_eq!(
            body["include"],
            serde_json::json!(["reasoning.encrypted_content"])
        );
    }

    #[test]
    fn compatible_provider_does_not_receive_openai_replay_fields() {
        let mut body = serde_json::json!({});
        assert!(!enable_encrypted_reasoning_replay(
            &mut body,
            "https://openai-compatible.example.com",
            "gpt-5.6",
        ));
        assert!(body.get("store").is_none());
        assert!(body.get("include").is_none());
    }

    #[test]
    fn output_item_capture_preserves_encrypted_reasoning() {
        let item = serde_json::json!({
            "id": "rs_123",
            "type": "reasoning",
            "encrypted_content": "encrypted-payload",
            "summary": [],
        });
        let mut pending = PendingQueue::new();
        let mut seen = std::collections::HashSet::new();
        enqueue_output_item_events(&mut pending, &mut seen, &item, false, true);

        assert_eq!(pending.len(), 1);
        match pending.pop_front().unwrap() {
            StreamEvent::RawContentBlock(raw) => assert_eq!(raw, item),
            event => panic!("expected raw output item, got {event:?}"),
        }
        enqueue_output_item_events(&mut pending, &mut seen, &item, false, true);
        assert!(pending.is_empty());
    }

    #[test]
    fn output_item_capture_is_disabled_for_other_responses_clients() {
        let item = serde_json::json!({
            "id": "rs_123",
            "type": "reasoning",
            "encrypted_content": "encrypted-payload",
            "summary": [],
        });
        let mut pending = PendingQueue::new();
        let mut seen = std::collections::HashSet::new();
        enqueue_output_item_events(&mut pending, &mut seen, &item, false, false);
        assert!(pending.is_empty());
    }

    // ---- Thinking/Reasoning parameter tests (3.12) ----

    #[test]
    fn reasoning_enabled_effort_high() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::High), "gpt-5.1");
        let val = result.unwrap();
        assert_eq!(val["effort"], "high");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn reasoning_enabled_effort_low() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Low), "gpt-5.1");
        let val = result.unwrap();
        assert_eq!(val["effort"], "low");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn reasoning_enabled_effort_none_defaults_to_medium() {
        let result = build_reasoning_params(true, None, "gpt-5.1");
        let val = result.unwrap();
        assert_eq!(val["effort"], "medium");
        assert_eq!(val["summary"], "auto");
    }

    #[test]
    fn explicit_reasoning_effort_is_honored_when_summary_is_disabled() {
        let result = build_reasoning_params(false, Some(ThinkingEffort::High), "gpt-5.4");
        let val = result.unwrap();
        assert_eq!(val["effort"], "high");
        assert!(val.get("summary").is_none());
    }

    #[test]
    fn reasoning_unconfigured_and_disabled_is_omitted() {
        assert!(build_reasoning_params(false, None, "gpt-5.6").is_none());
    }

    #[test]
    fn gpt_5_6_supports_none_xhigh_and_max() {
        let none = build_reasoning_params(true, Some(ThinkingEffort::None), "gpt-5.6-sol").unwrap();
        assert_eq!(none["effort"], "none");
        assert!(none.get("summary").is_none());

        let xhigh =
            build_reasoning_params(false, Some(ThinkingEffort::Xhigh), "gpt-5.6-terra").unwrap();
        assert_eq!(xhigh["effort"], "xhigh");

        let max = build_reasoning_params(false, Some(ThinkingEffort::Max), "openai/gpt-5.6-luna")
            .unwrap();
        assert_eq!(max["effort"], "max");
    }

    #[test]
    fn gpt_5_6_pro_and_context_are_serialized() {
        let val = build_reasoning_params_with_options(
            false,
            None,
            Some(ReasoningMode::Pro),
            Some(ReasoningContext::AllTurns),
            "gpt-5.6",
        )
        .unwrap();
        assert_eq!(val["mode"], "pro");
        assert_eq!(val["context"], "all_turns");
        assert!(val.get("effort").is_none());
    }

    #[test]
    fn reasoning_max_xhigh_supported() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.4");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_xhigh_gpt_5_5() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.5");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_xhigh_gpt_5_5_pro() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.5-pro");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_xhigh_gpt_5_4_pro() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.4-pro");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_xhigh_gpt_5_3_codex() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.3-codex");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_xhigh_gpt_5_2() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.2");
        assert_eq!(result.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn reasoning_max_downgraded_unsupported() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.1");
        assert_eq!(result.unwrap()["effort"], "high");
    }

    #[test]
    fn reasoning_max_downgraded_mini() {
        let result = build_reasoning_params(true, Some(ThinkingEffort::Max), "gpt-5.4-mini");
        assert_eq!(result.unwrap()["effort"], "high");
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

    #[test]
    fn convert_tool_result_with_image() {
        use crate::ImageContent;
        let msg = ChatMessage {
            role: ChatRole::Tool,
            content: "[Image: test.png]".to_string(),
            name: Some("read_file".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_1".to_string()),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: vec![ImageContent {
                data: b"fake_png_data".to_vec(),
                media_type: "image/png".to_string(),
                filename: Some("test.png".to_string()),
            }],
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        };
        let converted = convert_messages(&[msg], "agent", &OtherAgentRole::User);
        let obj = &converted[0];
        assert_eq!(obj["type"], "function_call_output");
        assert_eq!(obj["call_id"], "call_1");
        // output should be an array
        let output = obj["output"].as_array().unwrap();
        assert_eq!(output[0]["type"], "input_image");
        let image_url = output[0]["image_url"].as_str().unwrap();
        assert!(image_url.starts_with("data:image/png;base64,"));
        assert_eq!(output[1]["type"], "input_text");
        assert_eq!(output[1]["text"], "[Image: test.png]");
    }

    #[test]
    fn convert_tool_result_without_image() {
        let msg = ChatMessage {
            role: ChatRole::Tool,
            content: "file content".to_string(),
            name: Some("read_file".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_2".to_string()),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: vec![],
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        };
        let converted = convert_messages(&[msg], "agent", &OtherAgentRole::User);
        let obj = &converted[0];
        assert_eq!(obj["type"], "function_call_output");
        // output should be a plain string
        assert!(obj["output"].is_string());
        assert_eq!(obj["output"], "file content");
    }

    #[test]
    fn convert_assistant_with_thinking_blocks_is_ignored() {
        use crate::ThinkingBlock;
        let mut without = ChatMessage::text(
            ChatRole::Assistant,
            "answer".to_string(),
            Some("gpt".to_string()),
        );
        without.tool_calls = Some(vec![crate::ToolCallInfo {
            id: "tc_1".to_string(),
            name: "read_file".to_string(),
            arguments: r#"{"path":"a"}"#.to_string(),
            thought_signature: None,
        }]);
        let mut with = without.clone();
        with.thinking_blocks = vec![
            ThinkingBlock::Thinking {
                text: "reasoning".to_string(),
                signature: "sig".to_string(),
            },
            ThinkingBlock::Redacted {
                data: "opaque".to_string(),
            },
        ];

        let body_without = convert_messages(&[without], "gpt", &OtherAgentRole::User);
        let body_with = convert_messages(&[with], "gpt", &OtherAgentRole::User);
        assert_eq!(body_without, body_with);

        let serialized = serde_json::to_string(&body_with).unwrap();
        assert!(!serialized.contains("thinking"));
        assert!(!serialized.contains("redacted_thinking"));
        assert!(!serialized.contains("signature"));
    }
}
