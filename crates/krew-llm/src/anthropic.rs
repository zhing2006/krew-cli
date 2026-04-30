//! Anthropic Messages API (`POST /v1/messages`) implementation.
//!
//! Supports streaming with the Anthropic SSE event protocol, which uses typed
//! events (message_start, content_block_start, content_block_delta, etc.).

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::{SamplingConfig, ThinkingEffort};
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic Messages API client.
pub struct AnthropicClient {
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
    extra_headers: Vec<(String, String)>,
}

impl AnthropicClient {
    /// Create a new Anthropic Messages API client.
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
            extra_headers: config.extra_headers,
        }
    }
}

// ---------------------------------------------------------------------------
// max_tokens defaults by model
// ---------------------------------------------------------------------------

/// Get the default max_tokens for a given model name.
fn default_max_tokens(model: &str) -> u32 {
    let has = |s: &str| model.contains(s);
    if has("opus") && (has("4-6") || has("4-7")) {
        128_000
    } else if (has("sonnet") && (has("4-6") || has("4-7")))
        || (has("haiku") && has("4-5"))
        || (has("opus") && has("4-5"))
        || (has("sonnet") && has("4-5"))
    {
        64_000
    } else {
        // Older models (opus-4-0, opus-4-1, sonnet-3.5, etc.)
        32_000
    }
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Result of message conversion: system text + messages array.
pub(crate) struct ConvertedMessages {
    /// System prompt text (None if no system messages).
    pub system: Option<String>,
    /// Anthropic messages array.
    pub messages: Vec<serde_json::Value>,
}

/// Convert unified ChatMessages to Anthropic format.
///
/// - System messages → extracted to top-level `system` field
/// - User messages → `{role: "user", content: "..."}`
/// - Current agent's assistant → `{role: "assistant", content: "..."}`
/// - Other agents' assistant → role per `other_agent_role` with `[agent_name]` prefix
///
/// Consecutive same-role messages are merged.
pub(crate) fn convert_messages(
    messages: &[ChatMessage],
    self_agent_name: &str,
    other_agent_role: &OtherAgentRole,
) -> ConvertedMessages {
    // Collect system messages.
    let system_texts: Vec<&str> = messages
        .iter()
        .filter(|m| m.role == ChatRole::System)
        .map(|m| m.content.as_str())
        .collect();
    let system = if system_texts.is_empty() {
        None
    } else {
        Some(system_texts.join("\n\n"))
    };

    let mut result: Vec<serde_json::Value> = Vec::new();
    let mut pending: Vec<RoleContent> = Vec::new();

    for msg in messages.iter().filter(|m| m.role != ChatRole::System) {
        // Tool result messages: Anthropic uses role: "user" with tool_result content block.
        if msg.role == ChatRole::Tool {
            flush_pending_anthropic(&mut pending, &mut result);

            let tool_content = if msg.images.is_empty() {
                // Plain text content.
                serde_json::json!(msg.content)
            } else {
                // Multimodal: image blocks + text block.
                let mut blocks: Vec<serde_json::Value> = msg
                    .images
                    .iter()
                    .map(|img| {
                        serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": img.media_type,
                                "data": common::encode_base64(&img.data),
                            }
                        })
                    })
                    .collect();
                blocks.push(serde_json::json!({
                    "type": "text",
                    "text": msg.content,
                }));
                serde_json::json!(blocks)
            };

            let mut content_block = serde_json::json!({
                "type": "tool_result",
                "content": tool_content,
            });
            if let Some(ref id) = msg.tool_call_id {
                content_block["tool_use_id"] = serde_json::json!(id);
            }
            result.push(serde_json::json!({
                "role": "user",
                "content": [content_block],
            }));
            continue;
        }

        // Assistant messages with tool_calls: Anthropic uses tool_use content blocks.
        if let (ChatRole::Assistant, Some(tcs)) = (&msg.role, &msg.tool_calls) {
            flush_pending_anthropic(&mut pending, &mut result);

            let mut content_blocks: Vec<serde_json::Value> = Vec::new();
            if !msg.content.is_empty() {
                content_blocks.push(serde_json::json!({
                    "type": "text",
                    "text": msg.content,
                }));
            }
            for tc in tcs {
                let input: serde_json::Value =
                    serde_json::from_str(&tc.arguments).unwrap_or_default();
                content_blocks.push(serde_json::json!({
                    "type": "tool_use",
                    "id": tc.id,
                    "name": tc.name,
                    "input": input,
                }));
            }
            result.push(serde_json::json!({
                "role": "assistant",
                "content": content_blocks,
            }));
            continue;
        }

        // Regular messages.
        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        let role = match &msg.role {
            ChatRole::User | ChatRole::Tool => "user",
            ChatRole::Assistant if is_other_agent => match other_agent_role {
                OtherAgentRole::User => "user",
                OtherAgentRole::Assistant => "assistant",
            },
            ChatRole::Assistant => "assistant",
            ChatRole::System => unreachable!(),
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

    flush_pending_anthropic(&mut pending, &mut result);

    ConvertedMessages {
        system,
        messages: result,
    }
}

/// Merge and flush pending role-content items into the result vector.
fn flush_pending_anthropic(pending: &mut Vec<RoleContent>, result: &mut Vec<serde_json::Value>) {
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

/// Build sampling parameters for the Anthropic API.
///
/// Maps: temperature (clamped to 0-1), top_p, top_k, max_tokens (required),
/// stop_sequences.
/// Ignores: frequency_penalty, presence_penalty.
pub(crate) fn build_sampling_params(
    sampling: &SamplingConfig,
    model: &str,
    enable_thinking: bool,
) -> serde_json::Map<String, serde_json::Value> {
    let mut params = serde_json::Map::new();

    // max_tokens is required.
    let max_tokens = sampling
        .max_tokens
        .unwrap_or_else(|| default_max_tokens(model));
    params.insert("max_tokens".into(), serde_json::json!(max_tokens));

    // Temperature: clamp to 0-1 for Anthropic.
    if let Some(t) = sampling.temperature {
        let clamped = if enable_thinking {
            // When thinking is enabled, temperature must be 1.0.
            if (t - 1.0).abs() > f64::EPSILON {
                tracing::warn!("Anthropic: thinking enabled, overriding temperature {t} to 1.0");
            }
            1.0
        } else {
            t.clamp(0.0, 1.0)
        };
        params.insert("temperature".into(), serde_json::json!(clamped));
    } else if enable_thinking {
        // When thinking is enabled and no temperature set, don't set it
        // (API default is 1.0 which is what we want).
    }

    if let Some(p) = sampling.top_p {
        params.insert("top_p".into(), serde_json::json!(p));
    }
    if let Some(k) = sampling.top_k {
        params.insert("top_k".into(), serde_json::json!(k));
    }
    if let Some(ref stops) = sampling.stop_sequences {
        params.insert("stop_sequences".into(), serde_json::json!(stops));
    }
    // frequency_penalty, presence_penalty intentionally ignored.

    params
}

// ---------------------------------------------------------------------------
// Thinking parameter injection
// ---------------------------------------------------------------------------

/// Check if a model supports adaptive thinking (Opus 4.6+ / Sonnet 4.6+).
fn supports_adaptive(model: &str) -> bool {
    (model.contains("opus") || model.contains("sonnet"))
        && (model.contains("4-6") || model.contains("4-7"))
}

/// Check if a model supports the effort parameter (Opus 4.6, Sonnet 4.6, Opus 4.5).
fn supports_effort(model: &str) -> bool {
    supports_adaptive(model) || (model.contains("opus") && model.contains("4-5"))
}

/// Check if a model supports effort = "max" (Opus 4.6 / Sonnet 4.6).
fn supports_max_effort(model: &str) -> bool {
    supports_adaptive(model)
}

/// Build the thinking parameter for the request body.
pub(crate) fn build_thinking_params(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> Option<serde_json::Value> {
    if !enable_thinking {
        return None;
    }

    if supports_adaptive(model) {
        // Opus 4.6+ / Sonnet 4.6+: use adaptive thinking with summarized display
        // so the model emits thinking summary blocks the TUI can render.
        Some(serde_json::json!({
            "type": "adaptive",
            "display": "summarized",
        }))
    } else {
        // Older models: use enabled + budget_tokens.
        // Max maps to same budget as High (32768).
        let budget = match thinking_effort {
            Some(ThinkingEffort::Low) => 1024,
            Some(ThinkingEffort::High | ThinkingEffort::Max) => 32768,
            Some(ThinkingEffort::Medium) | None => 8192,
        };
        Some(serde_json::json!({
            "type": "enabled",
            "budget_tokens": budget,
        }))
    }
}

/// Build the output_config parameter for effort-capable models.
pub(crate) fn build_output_config(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> Option<serde_json::Value> {
    if !enable_thinking || !supports_effort(model) {
        return None;
    }

    thinking_effort.map(|effort| {
        let effort_str = if effort == ThinkingEffort::Max {
            if supports_max_effort(model) {
                "max"
            } else {
                // Downgrade to high on models that don't support max.
                "high"
            }
        } else {
            match effort {
                ThinkingEffort::Low => "low",
                ThinkingEffort::Medium => "medium",
                ThinkingEffort::High => "high",
                ThinkingEffort::Max => unreachable!(),
            }
        };
        serde_json::json!({"effort": effort_str})
    })
}

// ---------------------------------------------------------------------------
// Tool definition conversion
// ---------------------------------------------------------------------------

/// Convert ToolDefinitions to Anthropic format.
pub(crate) fn convert_tools(tools: &[ToolDefinition]) -> Vec<serde_json::Value> {
    tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SSE stream parsing
// ---------------------------------------------------------------------------

/// State machine for Anthropic SSE event parsing.
///
/// Tracks the current content block type, tool_use metadata, and accumulated
/// tool call arguments.
#[derive(Default)]
struct SseState {
    /// Input tokens from message_start.
    input_tokens: u32,
    /// Output tokens from message_delta.
    output_tokens: u32,
    /// Current content block type (if any).
    current_block_type: Option<String>,
    /// Current tool_use ID (if in a tool_use block).
    tool_id: String,
    /// Current tool_use name (if in a tool_use block).
    tool_name: String,
    /// Accumulated tool_use arguments JSON.
    tool_args: String,
}

/// Parse Anthropic SSE events into StreamEvents.
pub(crate) fn build_event_stream(
    response: reqwest::Response,
) -> impl Stream<Item = StreamEvent> + Send {
    use eventsource_stream::Eventsource;
    use futures::StreamExt;

    let byte_stream = response.bytes_stream();
    let sse_stream = byte_stream.eventsource();

    let state = SseState::default();

    futures::stream::unfold(
        (sse_stream, state, false),
        |(mut sse_stream, mut state, mut done)| async move {
            if done {
                return None;
            }

            loop {
                let next = sse_stream.next().await;
                match next {
                    Some(Ok(event)) => {
                        let event_type = event.event;
                        let data = event.data.trim().to_string();

                        match event_type.as_str() {
                            "message_start" => {
                                // Extract initial usage (input_tokens).
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(msg) = v.get("message")
                                    && let Some(usage) = msg.get("usage")
                                {
                                    state.input_tokens = usage
                                        .get("input_tokens")
                                        .and_then(|t| t.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                }
                                continue;
                            }

                            "content_block_start" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(block) = v.get("content_block")
                                {
                                    let block_type = block
                                        .get("type")
                                        .and_then(|t| t.as_str())
                                        .unwrap_or("")
                                        .to_string();

                                    if block_type == "tool_use" {
                                        state.tool_id = block
                                            .get("id")
                                            .and_then(|i| i.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        state.tool_name = block
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("")
                                            .to_string();
                                        state.tool_args.clear();
                                    }

                                    // Server-side tool (e.g. web_search): emit start,
                                    // accumulate input JSON, emit done at content_block_stop.
                                    if block_type == "server_tool_use" {
                                        state.tool_name = block
                                            .get("name")
                                            .and_then(|n| n.as_str())
                                            .unwrap_or("server_tool")
                                            .to_string();
                                        state.tool_args.clear();
                                        state.current_block_type = Some(block_type);
                                        return Some((
                                            StreamEvent::ServerToolStart {
                                                name: state.tool_name.clone(),
                                            },
                                            (sse_stream, state, done),
                                        ));
                                    }

                                    state.current_block_type = Some(block_type);
                                }
                                continue;
                            }

                            "content_block_delta" => {
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(delta) = v.get("delta")
                                {
                                    let delta_type =
                                        delta.get("type").and_then(|t| t.as_str()).unwrap_or("");

                                    match delta_type {
                                        "text_delta" => {
                                            if let Some(text) =
                                                delta.get("text").and_then(|t| t.as_str())
                                                && !text.is_empty()
                                            {
                                                return Some((
                                                    StreamEvent::TextDelta(text.to_string()),
                                                    (sse_stream, state, done),
                                                ));
                                            }
                                        }
                                        "thinking_delta" => {
                                            if let Some(thinking) =
                                                delta.get("thinking").and_then(|t| t.as_str())
                                                && !thinking.is_empty()
                                            {
                                                return Some((
                                                    StreamEvent::ThinkingDelta(
                                                        thinking.to_string(),
                                                    ),
                                                    (sse_stream, state, done),
                                                ));
                                            }
                                        }
                                        "input_json_delta" => {
                                            // Accumulate tool call arguments.
                                            if let Some(json) =
                                                delta.get("partial_json").and_then(|j| j.as_str())
                                            {
                                                state.tool_args.push_str(json);
                                            }
                                        }
                                        "signature_delta" => {
                                            // Internal — ignore.
                                        }
                                        _ => {}
                                    }
                                }
                                continue;
                            }

                            "content_block_stop" => {
                                // Emit tool call if this was a tool_use block.
                                if state.current_block_type.as_deref() == Some("tool_use") {
                                    let event = StreamEvent::ToolCall {
                                        id: state.tool_id.clone(),
                                        name: state.tool_name.clone(),
                                        arguments: state.tool_args.clone(),
                                        thought_signature: None,
                                    };
                                    state.current_block_type = None;
                                    state.tool_id.clear();
                                    state.tool_name.clear();
                                    state.tool_args.clear();
                                    return Some((event, (sse_stream, state, done)));
                                }
                                // Emit server tool done with accumulated query.
                                if state.current_block_type.as_deref() == Some("server_tool_use") {
                                    let query =
                                        serde_json::from_str::<serde_json::Value>(&state.tool_args)
                                            .ok()
                                            .and_then(|v| {
                                                v.get("query")
                                                    .and_then(|q| q.as_str())
                                                    .map(|s| s.to_string())
                                            });
                                    let event = StreamEvent::ServerToolDone {
                                        name: state.tool_name.clone(),
                                        query,
                                    };
                                    state.current_block_type = None;
                                    state.tool_name.clear();
                                    state.tool_args.clear();
                                    return Some((event, (sse_stream, state, done)));
                                }
                                state.current_block_type = None;
                                continue;
                            }

                            "message_delta" => {
                                // Extract cumulative usage (output_tokens).
                                if let Ok(v) = serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(usage) = v.get("usage")
                                {
                                    state.output_tokens = usage
                                        .get("output_tokens")
                                        .and_then(|t| t.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                }
                                continue;
                            }

                            "message_stop" => {
                                done = true;
                                let usage = Usage {
                                    prompt_tokens: state.input_tokens,
                                    completion_tokens: state.output_tokens,
                                    total_tokens: state.input_tokens + state.output_tokens,
                                };
                                return Some((StreamEvent::Done(usage), (sse_stream, state, done)));
                            }

                            "error" => {
                                done = true;
                                let msg = if let Ok(v) =
                                    serde_json::from_str::<serde_json::Value>(&data)
                                    && let Some(err) = v.get("error")
                                    && let Some(message) =
                                        err.get("message").and_then(|m| m.as_str())
                                {
                                    message.to_string()
                                } else {
                                    "unknown error".to_string()
                                };
                                return Some((StreamEvent::Error(msg), (sse_stream, state, done)));
                            }

                            // Ignore: ping, etc.
                            _ => continue,
                        }
                    }
                    Some(Err(e)) => {
                        done = true;
                        return Some((
                            StreamEvent::Error(format!("SSE stream error: {e}")),
                            (sse_stream, state, done),
                        ));
                    }
                    None => {
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

// ---------------------------------------------------------------------------
// LlmClient implementation
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl LlmClient for AnthropicClient {
    async fn chat_stream(
        &self,
        messages: &[ChatMessage],
        tools: &[ToolDefinition],
        sampling: &SamplingConfig,
        on_retry: Option<&(dyn Fn(common::RetryInfo) + Send + Sync)>,
    ) -> Result<Pin<Box<dyn Stream<Item = StreamEvent> + Send>>, LlmError> {
        let url = format!("{}/v1/messages", self.base_url);

        // Convert messages.
        let converted = convert_messages(messages, &self.agent_name, &self.other_agent_role);

        // Build request body.
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": converted.messages,
            "stream": true,
        });

        // Add system prompt if present.
        if let Some(system) = &converted.system {
            body["system"] = serde_json::json!(system);
        }

        // Merge sampling parameters (includes max_tokens).
        let sampling_params = build_sampling_params(sampling, &self.model, self.enable_thinking);
        for (k, v) in sampling_params {
            body[k] = v;
        }

        // Add thinking if enabled.
        if let Some(thinking) =
            build_thinking_params(self.enable_thinking, self.thinking_effort, &self.model)
        {
            body["thinking"] = thinking;
        }

        // Add output_config for adaptive thinking effort.
        if let Some(output_config) =
            build_output_config(self.enable_thinking, self.thinking_effort, &self.model)
        {
            body["output_config"] = output_config;
        }

        // Add tools if provided.
        if !tools.is_empty() || self.enable_web_search {
            let mut tool_list = convert_tools(tools);
            if self.enable_web_search {
                tool_list.push(serde_json::json!({
                    "type": "web_search_20250305",
                    "name": "web_search",
                }));
            }
            body["tools"] = serde_json::json!(tool_list);
        }

        // Send request with retry.
        let req_config = RequestConfig {
            http: &self.http,
            url: &url,
            body: &body,
            provider_name: "Anthropic",
        };
        let auth = AuthMode::Header("x-api-key", &self.api_key);
        let mut extra_headers = vec![(
            "anthropic-version".to_string(),
            ANTHROPIC_VERSION.to_string(),
        )];
        extra_headers.extend_from_slice(&self.extra_headers);
        let response = common::send_with_retry(
            &req_config,
            &auth,
            Some(&extra_headers),
            &self.retry_config,
            on_retry,
        )
        .await?;

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

    // ---- SSE parsing tests (5.8) ----

    #[test]
    fn sse_message_start_usage() {
        let data = r#"{"message":{"usage":{"input_tokens":100}}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let input = v["message"]["usage"]["input_tokens"].as_u64().unwrap();
        assert_eq!(input, 100);
    }

    #[test]
    fn sse_content_block_start_text() {
        let data = r#"{"content_block":{"type":"text","text":""}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["content_block"]["type"].as_str(), Some("text"));
    }

    #[test]
    fn sse_content_block_delta_text() {
        let data = r#"{"delta":{"type":"text_delta","text":"Hello"}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["delta"]["type"].as_str(), Some("text_delta"));
        assert_eq!(v["delta"]["text"].as_str(), Some("Hello"));
    }

    #[test]
    fn sse_content_block_delta_thinking() {
        let data = r#"{"delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["delta"]["type"].as_str(), Some("thinking_delta"));
        assert_eq!(v["delta"]["thinking"].as_str(), Some("Let me think..."));
    }

    #[test]
    fn sse_content_block_delta_signature() {
        let data = r#"{"delta":{"type":"signature_delta","signature":"abc"}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["delta"]["type"].as_str(), Some("signature_delta"));
        // signature_delta should be silently ignored (no StreamEvent).
    }

    #[test]
    fn sse_content_block_start_tool_use() {
        let data = r#"{"content_block":{"type":"tool_use","id":"tool_123","name":"read_file"}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let block = &v["content_block"];
        assert_eq!(block["type"].as_str(), Some("tool_use"));
        assert_eq!(block["id"].as_str(), Some("tool_123"));
        assert_eq!(block["name"].as_str(), Some("read_file"));
    }

    #[test]
    fn sse_input_json_delta() {
        let data = r#"{"delta":{"type":"input_json_delta","partial_json":"{\"path\":\""}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["delta"]["type"].as_str(), Some("input_json_delta"));
        assert_eq!(v["delta"]["partial_json"].as_str(), Some("{\"path\":\""));
    }

    #[test]
    fn sse_message_delta_usage() {
        let data = r#"{"delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":50}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["usage"]["output_tokens"].as_u64(), Some(50));
    }

    #[test]
    fn sse_error_event() {
        let data = r#"{"error":{"message":"rate limit exceeded"}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        assert_eq!(v["error"]["message"].as_str(), Some("rate limit exceeded"));
    }

    #[test]
    fn sse_empty_text_delta_ignored() {
        let data = r#"{"delta":{"type":"text_delta","text":""}}"#;
        let v: serde_json::Value = serde_json::from_str(data).unwrap();
        let text = v["delta"]["text"].as_str().unwrap();
        assert!(text.is_empty());
    }

    // ---- Message conversion tests (5.9) ----

    #[test]
    fn convert_system_to_top_level() {
        let messages = vec![ChatMessage::text(
            ChatRole::System,
            "you are helpful".to_string(),
            None,
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.system.as_deref(), Some("you are helpful"));
        assert!(result.messages.is_empty());
    }

    #[test]
    fn convert_multiple_system_merged() {
        let messages = vec![
            ChatMessage::text(ChatRole::System, "part 1".to_string(), None),
            ChatMessage::text(ChatRole::System, "part 2".to_string(), None),
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.system.as_deref(), Some("part 1\n\npart 2"));
    }

    #[test]
    fn convert_user_message() {
        let messages = vec![ChatMessage::text(ChatRole::User, "hello".to_string(), None)];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages[0]["role"], "user");
        assert_eq!(result.messages[0]["content"], "[user] hello");
    }

    #[test]
    fn convert_current_agent_assistant() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "my reply".to_string(),
            Some("agent1".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages[0]["role"], "assistant");
        assert_eq!(result.messages[0]["content"], "my reply");
    }

    #[test]
    fn convert_other_agent_to_user() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages[0]["role"], "user");
        assert_eq!(result.messages[0]["content"], "[agent2] other reply");
    }

    #[test]
    fn convert_other_agent_as_assistant() {
        let messages = vec![ChatMessage::text(
            ChatRole::Assistant,
            "other reply".to_string(),
            Some("agent2".to_string()),
        )];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::Assistant);
        assert_eq!(result.messages[0]["role"], "assistant");
        assert_eq!(result.messages[0]["content"], "[agent2] other reply");
    }

    #[test]
    fn convert_consecutive_user_merged() {
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
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0]["role"], "user");
        assert_eq!(
            result.messages[0]["content"],
            "[agentA] reply A\n\n[agentB] reply B"
        );
    }

    #[test]
    fn convert_three_agents_merged() {
        let messages = vec![
            ChatMessage::text(ChatRole::Assistant, "a".to_string(), Some("a1".to_string())),
            ChatMessage::text(ChatRole::Assistant, "b".to_string(), Some("a2".to_string())),
            ChatMessage::text(ChatRole::Assistant, "c".to_string(), Some("a3".to_string())),
        ];
        let result = convert_messages(&messages, "me", &OtherAgentRole::User);
        assert_eq!(result.messages.len(), 1);
    }

    #[test]
    fn convert_alternating_no_merge() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hi".to_string(), None),
            ChatMessage::text(
                ChatRole::Assistant,
                "hello".to_string(),
                Some("agent1".to_string()),
            ),
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn convert_system_in_middle_separated() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hi".to_string(), None),
            ChatMessage::text(ChatRole::System, "be nice".to_string(), None),
            ChatMessage::text(
                ChatRole::Assistant,
                "hello".to_string(),
                Some("agent1".to_string()),
            ),
        ];
        let result = convert_messages(&messages, "agent1", &OtherAgentRole::User);
        assert_eq!(result.system.as_deref(), Some("be nice"));
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn convert_empty_messages() {
        let result = convert_messages(&[], "agent1", &OtherAgentRole::User);
        assert!(result.system.is_none());
        assert!(result.messages.is_empty());
    }

    // ---- Sampling parameter tests (5.10) ----

    #[test]
    fn sampling_temperature_in_range() {
        let sampling = SamplingConfig {
            temperature: Some(0.5),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["temperature"], 0.5);
    }

    #[test]
    fn sampling_temperature_clamped() {
        let sampling = SamplingConfig {
            temperature: Some(1.5),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["temperature"], 1.0);
    }

    #[test]
    fn sampling_temperature_zero() {
        let sampling = SamplingConfig {
            temperature: Some(0.0),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["temperature"], 0.0);
    }

    #[test]
    fn sampling_max_tokens_user_value() {
        let sampling = SamplingConfig {
            max_tokens: Some(4096),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["max_tokens"], 4096);
    }

    #[test]
    fn sampling_max_tokens_default() {
        let params = build_sampling_params(&SamplingConfig::default(), "claude-opus-4-6", false);
        assert_eq!(params["max_tokens"], 128_000);
    }

    #[test]
    fn sampling_top_k() {
        let sampling = SamplingConfig {
            top_k: Some(40),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["top_k"], 40);
    }

    #[test]
    fn sampling_top_p() {
        let sampling = SamplingConfig {
            top_p: Some(0.9),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["top_p"], 0.9);
    }

    #[test]
    fn sampling_stop_sequences() {
        let sampling = SamplingConfig {
            stop_sequences: Some(vec!["STOP".into()]),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["stop_sequences"], serde_json::json!(["STOP"]));
    }

    #[test]
    fn sampling_frequency_penalty_ignored() {
        let sampling = SamplingConfig {
            frequency_penalty: Some(0.5),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert!(!params.contains_key("frequency_penalty"));
    }

    #[test]
    fn sampling_presence_penalty_ignored() {
        let sampling = SamplingConfig {
            presence_penalty: Some(0.3),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert!(!params.contains_key("presence_penalty"));
    }

    #[test]
    fn sampling_all_none_has_max_tokens() {
        let params = build_sampling_params(&SamplingConfig::default(), "unknown-model", false);
        assert!(params.contains_key("max_tokens"));
        assert_eq!(params["max_tokens"], 32_000);
    }

    // ---- max_tokens default tests (5.11) ----

    #[test]
    fn max_tokens_opus_4_6() {
        assert_eq!(default_max_tokens("claude-opus-4-6-20250801"), 128_000);
    }

    #[test]
    fn max_tokens_sonnet_4_6() {
        assert_eq!(default_max_tokens("claude-sonnet-4-6-20250801"), 64_000);
    }

    #[test]
    fn max_tokens_haiku_4_5() {
        assert_eq!(default_max_tokens("claude-haiku-4-5-20251001"), 64_000);
    }

    #[test]
    fn max_tokens_opus_4_5() {
        assert_eq!(default_max_tokens("claude-opus-4-5-20250901"), 64_000);
    }

    #[test]
    fn max_tokens_sonnet_4_5() {
        assert_eq!(default_max_tokens("claude-sonnet-4-5-20250901"), 64_000);
    }

    #[test]
    fn max_tokens_older_models() {
        assert_eq!(default_max_tokens("claude-opus-4-0-20250301"), 32_000);
        assert_eq!(default_max_tokens("claude-opus-4-1-20250501"), 32_000);
    }

    #[test]
    fn max_tokens_unknown() {
        assert_eq!(default_max_tokens("unknown-model"), 32_000);
    }

    #[test]
    fn max_tokens_user_overrides_default() {
        let sampling = SamplingConfig {
            max_tokens: Some(4096),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", false);
        assert_eq!(params["max_tokens"], 4096);
    }

    // ---- Authentication tests (5.12) ----

    #[test]
    fn auth_x_api_key() {
        let auth = AuthMode::Header("x-api-key", "sk-test-key");
        assert!(matches!(auth, AuthMode::Header("x-api-key", "sk-test-key")));
    }

    #[test]
    fn auth_anthropic_version_header() {
        assert_eq!(ANTHROPIC_VERSION, "2023-06-01");
    }

    // ---- Thinking parameter tests (5.13) ----

    #[test]
    fn thinking_opus_4_6_adaptive() {
        let result = build_thinking_params(true, None, "claude-opus-4-6-20250801");
        let val = result.unwrap();
        assert_eq!(val["type"], "adaptive");
        assert_eq!(val["display"], "summarized");
        assert!(val.get("budget_tokens").is_none());
    }

    #[test]
    fn thinking_opus_4_6_with_effort() {
        let thinking = build_thinking_params(true, Some(ThinkingEffort::High), "claude-opus-4-6");
        assert_eq!(thinking.unwrap()["type"], "adaptive");

        let output = build_output_config(true, Some(ThinkingEffort::High), "claude-opus-4-6");
        assert_eq!(output.unwrap()["effort"], "high");
    }

    #[test]
    fn thinking_sonnet_4_6_with_effort() {
        let thinking = build_thinking_params(true, Some(ThinkingEffort::Low), "claude-sonnet-4-6");
        assert_eq!(thinking.unwrap()["type"], "adaptive");

        let output = build_output_config(true, Some(ThinkingEffort::Low), "claude-sonnet-4-6");
        assert_eq!(output.unwrap()["effort"], "low");
    }

    #[test]
    fn thinking_old_model_budget_high() {
        let result =
            build_thinking_params(true, Some(ThinkingEffort::High), "claude-opus-4-5-20250901");
        let val = result.unwrap();
        assert_eq!(val["type"], "enabled");
        assert_eq!(val["budget_tokens"], 32768);
    }

    #[test]
    fn thinking_old_model_budget_medium() {
        let result = build_thinking_params(true, Some(ThinkingEffort::Medium), "claude-opus-4-5");
        assert_eq!(result.unwrap()["budget_tokens"], 8192);
    }

    #[test]
    fn thinking_old_model_budget_low() {
        let result = build_thinking_params(true, Some(ThinkingEffort::Low), "claude-opus-4-5");
        assert_eq!(result.unwrap()["budget_tokens"], 1024);
    }

    #[test]
    fn thinking_old_model_budget_none_default() {
        let result = build_thinking_params(true, None, "claude-opus-4-5");
        assert_eq!(result.unwrap()["budget_tokens"], 8192);
    }

    #[test]
    fn thinking_disabled() {
        let result = build_thinking_params(false, Some(ThinkingEffort::High), "claude-opus-4-6");
        assert!(result.is_none());
    }

    #[test]
    fn thinking_opus_4_6_max_effort() {
        let thinking = build_thinking_params(true, Some(ThinkingEffort::Max), "claude-opus-4-6");
        assert_eq!(thinking.unwrap()["type"], "adaptive");

        let output = build_output_config(true, Some(ThinkingEffort::Max), "claude-opus-4-6");
        assert_eq!(output.unwrap()["effort"], "max");
    }

    #[test]
    fn thinking_sonnet_4_6_max_effort() {
        let output = build_output_config(true, Some(ThinkingEffort::Max), "claude-sonnet-4-6");
        assert_eq!(output.unwrap()["effort"], "max");
    }

    #[test]
    fn thinking_opus_4_7_adaptive() {
        let result = build_thinking_params(true, None, "claude-opus-4-7");
        let val = result.unwrap();
        assert_eq!(val["type"], "adaptive");
        assert_eq!(val["display"], "summarized");
        assert!(val.get("budget_tokens").is_none());
    }

    #[test]
    fn thinking_opus_4_7_with_effort() {
        let thinking = build_thinking_params(true, Some(ThinkingEffort::High), "claude-opus-4-7");
        assert_eq!(thinking.unwrap()["type"], "adaptive");

        let output = build_output_config(true, Some(ThinkingEffort::High), "claude-opus-4-7");
        assert_eq!(output.unwrap()["effort"], "high");
    }

    #[test]
    fn thinking_opus_4_7_max_effort() {
        let output = build_output_config(true, Some(ThinkingEffort::Max), "claude-opus-4-7");
        assert_eq!(output.unwrap()["effort"], "max");
    }

    #[test]
    fn max_tokens_opus_4_7() {
        assert_eq!(default_max_tokens("claude-opus-4-7"), 128_000);
    }

    #[test]
    fn thinking_opus_4_5_effort_supported() {
        let output = build_output_config(
            true,
            Some(ThinkingEffort::Medium),
            "claude-opus-4-5-20250901",
        );
        assert_eq!(output.unwrap()["effort"], "medium");
    }

    #[test]
    fn thinking_opus_4_5_max_downgraded() {
        let thinking =
            build_thinking_params(true, Some(ThinkingEffort::Max), "claude-opus-4-5-20250901");
        let val = thinking.unwrap();
        assert_eq!(val["type"], "enabled");
        assert_eq!(val["budget_tokens"], 32768);

        let output =
            build_output_config(true, Some(ThinkingEffort::Max), "claude-opus-4-5-20250901");
        assert_eq!(output.unwrap()["effort"], "high");
    }

    #[test]
    fn thinking_sonnet_4_5_no_effort() {
        let output = build_output_config(
            true,
            Some(ThinkingEffort::High),
            "claude-sonnet-4-5-20250901",
        );
        assert!(output.is_none());
    }

    #[test]
    fn thinking_haiku_4_5_no_effort() {
        let output = build_output_config(
            true,
            Some(ThinkingEffort::High),
            "claude-haiku-4-5-20251001",
        );
        assert!(output.is_none());
    }

    #[test]
    fn thinking_legacy_max_budget() {
        let result = build_thinking_params(
            true,
            Some(ThinkingEffort::Max),
            "claude-sonnet-4-5-20250901",
        );
        let val = result.unwrap();
        assert_eq!(val["type"], "enabled");
        assert_eq!(val["budget_tokens"], 32768);

        let output = build_output_config(
            true,
            Some(ThinkingEffort::Max),
            "claude-sonnet-4-5-20250901",
        );
        assert!(output.is_none());
    }

    #[test]
    fn thinking_temperature_forced_to_1() {
        let sampling = SamplingConfig {
            temperature: Some(0.5),
            ..Default::default()
        };
        let params = build_sampling_params(&sampling, "claude-opus-4-6", true);
        assert_eq!(params["temperature"], 1.0);
    }

    #[test]
    fn thinking_temperature_not_set_no_override() {
        let params = build_sampling_params(&SamplingConfig::default(), "claude-opus-4-6", true);
        // When thinking is enabled and no temperature set, don't add it.
        assert!(!params.contains_key("temperature"));
    }

    // ---- Tool definition conversion tests (5.14) ----

    #[test]
    fn convert_single_tool() {
        let tools = vec![ToolDefinition {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
        }];
        let result = convert_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["name"], "read_file");
        assert_eq!(result[0]["description"], "Read a file");
        // Anthropic uses input_schema, not parameters.
        assert!(result[0]["input_schema"].is_object());
        assert!(result[0].get("parameters").is_none());
    }

    #[test]
    fn convert_multiple_tools() {
        let tools = vec![
            ToolDefinition {
                name: "a".to_string(),
                description: "A".to_string(),
                parameters: serde_json::json!({}),
            },
            ToolDefinition {
                name: "b".to_string(),
                description: "B".to_string(),
                parameters: serde_json::json!({}),
            },
        ];
        let result = convert_tools(&tools);
        assert_eq!(result.len(), 2);
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
        tool_list.push(serde_json::json!({
            "type": "web_search_20250305",
            "name": "web_search",
        }));
        assert_eq!(tool_list.len(), 2);
        assert_eq!(tool_list[0]["name"], "read_file");
        assert_eq!(tool_list[1]["type"], "web_search_20250305");
        assert_eq!(tool_list[1]["name"], "web_search");
    }

    #[test]
    fn web_search_only_no_function_tools() {
        let mut tool_list = convert_tools(&[]);
        tool_list.push(serde_json::json!({
            "type": "web_search_20250305",
            "name": "web_search",
        }));
        assert_eq!(tool_list.len(), 1);
        assert_eq!(tool_list[0]["type"], "web_search_20250305");
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
        };
        let converted = convert_messages(&[msg], "agent", &OtherAgentRole::User);
        let tool_result = &converted.messages[0]["content"][0];
        assert_eq!(tool_result["type"], "tool_result");
        assert_eq!(tool_result["tool_use_id"], "call_1");
        // content should be an array with image + text blocks
        let content = &tool_result["content"];
        assert!(content.is_array());
        let blocks = content.as_array().unwrap();
        assert_eq!(blocks[0]["type"], "image");
        assert_eq!(blocks[0]["source"]["type"], "base64");
        assert_eq!(blocks[0]["source"]["media_type"], "image/png");
        assert_eq!(blocks[1]["type"], "text");
        assert_eq!(blocks[1]["text"], "[Image: test.png]");
    }

    #[test]
    fn convert_tool_result_without_image() {
        let msg = ChatMessage {
            role: ChatRole::Tool,
            content: "file content here".to_string(),
            name: Some("read_file".to_string()),
            tool_calls: None,
            tool_call_id: Some("call_2".to_string()),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: vec![],
        };
        let converted = convert_messages(&[msg], "agent", &OtherAgentRole::User);
        let tool_result = &converted.messages[0]["content"][0];
        assert_eq!(tool_result["type"], "tool_result");
        // content should be a plain string, not an array
        assert!(tool_result["content"].is_string());
        assert_eq!(tool_result["content"], "file content here");
    }
}
