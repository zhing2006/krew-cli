//! OpenAI Chat Completions API (`POST /v1/chat/completions`) implementation.

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::SamplingConfig;
use krew_config::ThinkingEffort;
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
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    enable_web_search: bool,
    extra_headers: Vec<(String, String)>,
}

impl OpenAiChatClient {
    /// Create a new OpenAI Chat Completions client.
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
            other_agent_role: config.other_agent_role,
            retry_config: config.retry_config,
            enable_thinking: config.enable_thinking,
            thinking_effort: config.thinking_effort,
            enable_web_search: config.enable_web_search,
            extra_headers: config.extra_headers,
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
// Thinking/reasoning parameter injection
// ---------------------------------------------------------------------------

/// Return the provider-independent model slug.
fn model_slug(model: &str) -> &str {
    model.rsplit('/').next().unwrap_or(model)
}

/// Check whether the model slug matches an exact family name or a dashed variant.
fn slug_matches_family(model: &str, family: &str) -> bool {
    let slug = model_slug(model).to_ascii_lowercase();
    slug == family
        || slug
            .strip_prefix(family)
            .is_some_and(|rest| rest.starts_with('-'))
}

/// Check if a model supports xhigh reasoning effort (whitelist).
///
/// Matches exact model name or model name with date suffix (e.g. "gpt-5.4-20260101").
fn supports_xhigh(model: &str) -> bool {
    const XHIGH_MODELS: &[&str] = &[
        "gpt-5.5",
        "gpt-5.5-pro",
        "gpt-5.4",
        "gpt-5.4-pro",
        "gpt-5.3-codex",
        "gpt-5.2",
    ];
    let slug = model_slug(model).to_ascii_lowercase();
    XHIGH_MODELS.iter().any(|m| {
        slug == *m
            || (slug.starts_with(m)
                && slug.as_bytes().get(m.len()) == Some(&b'-')
                && slug
                    .as_bytes()
                    .get(m.len() + 1)
                    .is_some_and(|c| c.is_ascii_digit()))
    })
}

/// Check if a model uses DeepSeek V4 thinking controls.
fn is_deepseek_v4(model: &str) -> bool {
    slug_matches_family(model, "deepseek-v4")
}

/// Check if a model uses Kimi K2.6 thinking controls.
fn is_kimi_k26(model: &str) -> bool {
    slug_matches_family(model, "kimi-k2.6")
}

/// Map `enable_thinking` + `thinking_effort` to the `reasoning_effort` string
/// used by OpenAI Chat Completions API (and LiteLLM proxy).
fn build_reasoning_effort(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> Option<&'static str> {
    if !enable_thinking {
        return None;
    }
    if is_kimi_k26(model) {
        return None;
    }
    if is_deepseek_v4(model) {
        return Some(match thinking_effort {
            Some(ThinkingEffort::Max) => "max",
            Some(ThinkingEffort::Low)
            | Some(ThinkingEffort::Medium)
            | Some(ThinkingEffort::High)
            | None => "high",
        });
    }
    Some(match thinking_effort {
        Some(ThinkingEffort::Low) => "low",
        Some(ThinkingEffort::High) => "high",
        Some(ThinkingEffort::Max) => {
            if supports_xhigh(model) {
                "xhigh"
            } else {
                "high"
            }
        }
        Some(ThinkingEffort::Medium) | None => "medium",
    })
}

/// Build OpenAI-compatible thinking-control fields for hybrid-thinking models.
///
/// DeepSeek V4 and Kimi K2.6 are served via two incompatible OpenAI-compatible
/// APIs that disagree on the field name:
/// - DeepSeek official / Moonshot official: `thinking: {"type": "enabled"|"disabled"}`.
/// - Aliyun DashScope / Bailian: top-level `enable_thinking: <bool>` (passed via
///   OpenAI SDK's `extra_body`).
///
/// We send both so the same config works across LiteLLM routes — backends that
/// don't recognize a field silently ignore it.
fn build_compatible_thinking_fields(
    model: &str,
    enable_thinking: bool,
) -> Vec<(&'static str, serde_json::Value)> {
    if !(is_deepseek_v4(model) || is_kimi_k26(model)) {
        return Vec::new();
    }
    vec![
        (
            "thinking",
            serde_json::json!({
                "type": if enable_thinking { "enabled" } else { "disabled" },
            }),
        ),
        ("enable_thinking", serde_json::json!(enable_thinking)),
    ]
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

    // Extract grounding metadata from LiteLLM proxy (Gemini).
    // Empty `[{}]` means search in progress; non-empty with webSearchQueries means done.
    let grounding = v
        .get("vertex_ai_grounding_metadata")
        .and_then(|arr| arr.as_array())
        .and_then(|arr| arr.first())
        .and_then(|gm| {
            let obj = gm.as_object()?;
            if obj.is_empty() {
                Some(None) // Search in progress
            } else {
                // Search complete — extract first query if available.
                let query = obj
                    .get("webSearchQueries")
                    .and_then(|q| q.as_array())
                    .and_then(|arr| arr.first())
                    .and_then(|q| q.as_str())
                    .map(|s| s.to_string());
                Some(query)
            }
        });

    if choices.is_empty() {
        // Usage-only chunk (the last chunk before [DONE]).
        return Some(SseChunk {
            event: None,
            usage,
            tool_call_delta: None,
            grounding,
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
            grounding,
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
            grounding,
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
            grounding,
        });
    }

    Some(SseChunk {
        event: None,
        usage,
        tool_call_delta: None,
        grounding,
    })
}

struct SseChunk {
    event: Option<StreamEvent>,
    usage: Option<Usage>,
    /// Partial tool call data to accumulate (OpenAI streams tool calls incrementally).
    tool_call_delta: Option<ToolCallDelta>,
    /// Grounding metadata from LiteLLM proxy (Gemini web search via vertex_ai_grounding_metadata).
    /// `Some(None)` = empty metadata (search in progress),
    /// `Some(Some(query))` = search complete with query.
    grounding: Option<Option<String>>,
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

        // Add reasoning_effort if thinking is enabled (for proxies like LiteLLM).
        if let Some(effort) =
            build_reasoning_effort(self.enable_thinking, self.thinking_effort, &self.model)
        {
            body["reasoning_effort"] = serde_json::json!(effort);
        }
        for (key, value) in build_compatible_thinking_fields(&self.model, self.enable_thinking) {
            body[key] = value;
        }

        // Add web_search_options if enabled (OpenAI native + LiteLLM proxy).
        if self.enable_web_search {
            body["web_search_options"] = serde_json::json!({
                "search_context_size": "medium"
            });
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

        // Convert response into SSE event stream.
        let stream = build_event_stream(response);

        Ok(Box::pin(stream))
    }
}

/// Known server-side tool names (executed by the provider, not the client).
const SERVER_TOOL_NAMES: &[&str] = &["web_search"];

/// Extract the search query from accumulated server tool arguments JSON.
fn extract_server_tool_query(args: &str) -> Option<String> {
    serde_json::from_str::<serde_json::Value>(args)
        .ok()
        .and_then(|v| v.get("query")?.as_str().map(|s| s.to_string()))
}

/// Convert an HTTP response into a `Stream<Item = StreamEvent>`.
///
/// Tool calls are streamed incrementally by OpenAI (first chunk has id + name,
/// subsequent chunks carry partial arguments). This function accumulates them
/// and emits complete `StreamEvent::ToolCall` events only when `[DONE]` arrives.
///
/// Server-side tools (e.g. `web_search` via LiteLLM) are detected and emitted
/// as `ServerToolStart`/`ServerToolDone` instead of `ToolCall`.
///
/// Gemini grounding metadata (`vertex_ai_grounding_metadata`) is also handled.
fn build_event_stream(response: reqwest::Response) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;
    use std::collections::VecDeque;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    let state = ChatStreamState {
        usage: Usage::default(),
        tool_calls_accum: Vec::new(),
        pending: VecDeque::new(),
        done: false,
        // Gemini grounding: 0=not seen, 1=started, 2=done
        grounding_state: 0u8,
        // Server-side tool (Claude web_search via LiteLLM):
        // (name, accumulated_arguments)
        server_tool: None,
    };

    futures::stream::unfold((sse_stream, state), |(mut sse_stream, mut st)| async move {
        // Drain pending events first.
        if let Some(event) = st.pending.pop_front() {
            return Some((event, (sse_stream, st)));
        }

        if st.done {
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
                            // [DONE] — flush server tool if still pending.
                            st.done = true;
                            if let Some((name, args)) = st.server_tool.take() {
                                let query = extract_server_tool_query(&args);
                                st.pending
                                    .push_back(StreamEvent::ServerToolDone { name, query });
                            }
                            // Emit accumulated client-side tool calls.
                            for (id, name, args) in st.tool_calls_accum.drain(..) {
                                st.pending.push_back(StreamEvent::ToolCall {
                                    id,
                                    name,
                                    arguments: args,
                                    thought_signature: None,
                                });
                            }
                            st.pending.push_back(StreamEvent::Done(st.usage.clone()));

                            if let Some(event) = st.pending.pop_front() {
                                return Some((event, (sse_stream, st)));
                            }
                            return None;
                        }
                        Some(chunk) => {
                            // Accumulate usage if present.
                            if let Some(u) = chunk.usage {
                                st.usage = u;
                            }

                            // Handle Gemini grounding metadata (via LiteLLM proxy).
                            // Empty metadata `[{}]` just means the grounding feature
                            // is active, NOT that a search actually happened.  Only
                            // emit ServerToolStart/Done when non-empty metadata with
                            // actual search queries arrives.
                            if let Some(grounding) = chunk.grounding {
                                match grounding {
                                    None if st.grounding_state == 0 => {
                                        // Empty metadata — grounding feature active but
                                        // no search confirmed yet.  Record it silently.
                                        st.grounding_state = 1;
                                        // Fall through to process any event/delta normally.
                                    }
                                    Some(query) if st.grounding_state < 2 => {
                                        // Non-empty metadata — search actually happened.
                                        st.grounding_state = 2;
                                        // Emit start + done together.
                                        st.pending.push_back(StreamEvent::ServerToolDone {
                                            name: "web_search".to_string(),
                                            query: Some(query),
                                        });
                                        if let Some(ev) = chunk.event {
                                            st.pending.push_back(ev);
                                        }
                                        return Some((
                                            StreamEvent::ServerToolStart {
                                                name: "web_search".to_string(),
                                            },
                                            (sse_stream, st),
                                        ));
                                    }
                                    _ => {
                                        // Already handled or no-op — fall through.
                                    }
                                }
                            }

                            // Handle tool call deltas.
                            if let Some(delta) = chunk.tool_call_delta {
                                // Check if this is a server-side tool.
                                let is_server_tool = (!delta.name.is_empty()
                                    && SERVER_TOOL_NAMES.contains(&delta.name.as_str()))
                                    || st.server_tool.is_some();

                                if is_server_tool {
                                    if st.server_tool.is_none() {
                                        // First chunk of a server tool — emit start.
                                        st.server_tool =
                                            Some((delta.name.clone(), delta.arguments.clone()));
                                        return Some((
                                            StreamEvent::ServerToolStart { name: delta.name },
                                            (sse_stream, st),
                                        ));
                                    }
                                    // Continuation chunk — accumulate arguments.
                                    if let Some((_, ref mut args)) = st.server_tool {
                                        args.push_str(&delta.arguments);
                                    }
                                    continue;
                                }

                                // Regular client-side tool call — accumulate.
                                let idx = delta.index as usize;
                                if idx >= st.tool_calls_accum.len() {
                                    st.tool_calls_accum.resize(
                                        idx + 1,
                                        (String::new(), String::new(), String::new()),
                                    );
                                }
                                let entry = &mut st.tool_calls_accum[idx];
                                if !delta.id.is_empty() {
                                    entry.0 = delta.id;
                                }
                                if !delta.name.is_empty() {
                                    entry.1 = delta.name;
                                }
                                entry.2.push_str(&delta.arguments);
                                continue;
                            }

                            // Before emitting text/thinking, flush pending server tool.
                            if let Some(ev) = &chunk.event
                                && matches!(
                                    ev,
                                    StreamEvent::TextDelta(_) | StreamEvent::ThinkingDelta(_)
                                )
                                && let Some((name, args)) = st.server_tool.take()
                            {
                                let query = extract_server_tool_query(&args);
                                st.pending.push_back(ev.clone());
                                return Some((
                                    StreamEvent::ServerToolDone { name, query },
                                    (sse_stream, st),
                                ));
                            }

                            // Emit stream event if present.
                            if let Some(event) = chunk.event {
                                return Some((event, (sse_stream, st)));
                            }
                            // No event (e.g. usage-only chunk) — continue.
                            continue;
                        }
                    }
                }
                Some(Err(e)) => {
                    st.done = true;
                    return Some((
                        StreamEvent::Error(format!("SSE stream error: {e}")),
                        (sse_stream, st),
                    ));
                }
                None => {
                    st.done = true;
                    return Some((
                        StreamEvent::Error("stream interrupted".into()),
                        (sse_stream, st),
                    ));
                }
            }
        }
    })
}

/// Internal state for the chat completions SSE stream processor.
struct ChatStreamState {
    usage: Usage,
    /// Accumulated client-side tool call deltas: (id, name, arguments).
    tool_calls_accum: Vec<(String, String, String)>,
    /// Queue for emitting multiple events at once.
    pending: std::collections::VecDeque<StreamEvent>,
    done: bool,
    /// Gemini grounding state: 0=not seen, 1=started, 2=done.
    grounding_state: u8,
    /// Pending server-side tool: (name, accumulated_arguments).
    server_tool: Option<(String, String)>,
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
        assert_eq!(result[0]["content"], "[user] hello");
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

    // ---- Reasoning effort tests ----

    #[test]
    fn reasoning_effort_max_xhigh_supported() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.4");
        assert_eq!(result, Some("xhigh"));
    }

    #[test]
    fn reasoning_effort_max_xhigh_gpt_5_5() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.5");
        assert_eq!(result, Some("xhigh"));
    }

    #[test]
    fn reasoning_effort_max_xhigh_gpt_5_5_pro() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.5-pro");
        assert_eq!(result, Some("xhigh"));
    }

    #[test]
    fn reasoning_effort_max_xhigh_gpt_5_4_pro() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.4-pro");
        assert_eq!(result, Some("xhigh"));
    }

    #[test]
    fn reasoning_effort_max_xhigh_gpt_5_2() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.2");
        assert_eq!(result, Some("xhigh"));
    }

    #[test]
    fn reasoning_effort_max_downgraded_gpt_5_1() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.1");
        assert_eq!(result, Some("high"));
    }

    #[test]
    fn reasoning_effort_max_downgraded_mini() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::Max), "gpt-5.4-mini");
        assert_eq!(result, Some("high"));
    }

    #[test]
    fn reasoning_effort_high_unchanged() {
        let result = build_reasoning_effort(true, Some(ThinkingEffort::High), "gpt-5.1");
        assert_eq!(result, Some("high"));
    }

    #[test]
    fn reasoning_effort_deepseek_v4_max_maps_to_max() {
        let pro = build_reasoning_effort(true, Some(ThinkingEffort::Max), "deepseek-v4-pro");
        let flash = build_reasoning_effort(true, Some(ThinkingEffort::Max), "deepseek-v4-flash");
        let dashscope =
            build_reasoning_effort(true, Some(ThinkingEffort::Max), "dashscope/deepseek-v4-pro");

        assert_eq!(pro, Some("max"));
        assert_eq!(flash, Some("max"));
        assert_eq!(dashscope, Some("max"));
    }

    #[test]
    fn reasoning_effort_deepseek_v4_non_max_maps_to_high() {
        assert_eq!(
            build_reasoning_effort(true, Some(ThinkingEffort::Low), "deepseek-v4-pro"),
            Some("high")
        );
        assert_eq!(
            build_reasoning_effort(true, Some(ThinkingEffort::Medium), "deepseek-v4-flash"),
            Some("high")
        );
        assert_eq!(
            build_reasoning_effort(true, Some(ThinkingEffort::High), "deepseek-v4-pro"),
            Some("high")
        );
        assert_eq!(
            build_reasoning_effort(true, None, "DeepSeek-V4-Flash"),
            Some("high")
        );
    }

    fn enabled_pair(enabled: bool) -> Vec<(&'static str, serde_json::Value)> {
        vec![
            (
                "thinking",
                serde_json::json!({
                    "type": if enabled { "enabled" } else { "disabled" },
                }),
            ),
            ("enable_thinking", serde_json::json!(enabled)),
        ]
    }

    #[test]
    fn deepseek_v4_thinking_fields_enabled() {
        assert_eq!(
            build_compatible_thinking_fields("deepseek-v4-pro", true),
            enabled_pair(true)
        );
    }

    #[test]
    fn deepseek_v4_thinking_fields_disabled() {
        assert_eq!(
            build_compatible_thinking_fields("deepseek-v4-flash", false),
            enabled_pair(false)
        );
    }

    #[test]
    fn deepseek_v4_thinking_fields_match_provider_slug() {
        assert_eq!(
            build_compatible_thinking_fields("dashscope/deepseek-v4-pro", true),
            enabled_pair(true)
        );
    }

    #[test]
    fn kimi_k26_thinking_fields_enabled() {
        assert_eq!(
            build_compatible_thinking_fields("kimi-k2.6", true),
            enabled_pair(true)
        );
        assert_eq!(
            build_compatible_thinking_fields("moonshotai/kimi-k2.6", true),
            enabled_pair(true)
        );
    }

    #[test]
    fn kimi_k26_thinking_fields_disabled() {
        assert_eq!(
            build_compatible_thinking_fields("moonshotai/kimi-k2.6", false),
            enabled_pair(false)
        );
    }

    #[test]
    fn kimi_k26_does_not_send_reasoning_effort() {
        let direct = build_reasoning_effort(true, Some(ThinkingEffort::High), "kimi-k2.6");
        let provider =
            build_reasoning_effort(true, Some(ThinkingEffort::Max), "moonshotai/kimi-k2.6");

        assert!(direct.is_none());
        assert!(provider.is_none());
    }

    #[test]
    fn non_hybrid_thinking_models_have_no_thinking_fields() {
        assert!(build_compatible_thinking_fields("gpt-5.4", false).is_empty());
        assert!(build_compatible_thinking_fields("claude-opus-4-7", true).is_empty());
        assert!(build_compatible_thinking_fields("deepseek-v3.1", true).is_empty());
    }

    #[test]
    fn reasoning_effort_disabled() {
        let result = build_reasoning_effort(false, Some(ThinkingEffort::Max), "gpt-5.4");
        assert!(result.is_none());
    }

    #[test]
    fn convert_tool_result_with_image_degrades() {
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
        assert_eq!(obj["role"], "tool");
        // Should degrade to text only, ignoring images
        assert_eq!(obj["content"], "[Image: test.png]");
        assert_eq!(obj["tool_call_id"], "call_1");
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
