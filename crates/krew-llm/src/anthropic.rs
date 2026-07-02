//! Anthropic Messages API (`POST /v1/messages`) implementation.
//!
//! Supports streaming with the Anthropic SSE event protocol, which uses typed
//! events (message_start, content_block_start, content_block_delta, etc.).

use crate::common::{self, AuthMode, RequestConfig, RoleContent, merge_consecutive_same_role};
use crate::{
    ChatMessage, ChatRole, LlmClient, LlmClientConfig, LlmError, StreamEvent, ThinkingBlock,
    ToolDefinition, Usage,
};
use futures::Stream;
use krew_config::OtherAgentRole;
use krew_config::RetryConfig;
use krew_config::{SamplingConfig, ThinkingEffort};
use std::pin::Pin;

const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";
pub(crate) const ANTHROPIC_VERSION: &str = "2023-06-01";

// Anthropic SSE / Messages API protocol strings shared between the request
// serialiser (thinking_blocks_to_json) and the SSE state machine.
const BLOCK_TYPE_THINKING: &str = "thinking";
const BLOCK_TYPE_REDACTED_THINKING: &str = "redacted_thinking";
const DELTA_TYPE_THINKING: &str = "thinking_delta";
const DELTA_TYPE_SIGNATURE: &str = "signature_delta";

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
// Model version parsing
// ---------------------------------------------------------------------------

/// Parse the `(major, minor)` version from a Claude model name such as
/// `claude-opus-4-8`, `claude-opus-4-8-20260301`, or the Vertex form
/// `claude-opus-4-8@20260301`. The newer naming scheme drops the minor
/// segment (e.g. `claude-sonnet-5`), which is parsed as `(5, 0)`. Returns
/// `None` for names without a `<family>-<major>[-<minor>]` segment (e.g.
/// legacy `claude-3-5-sonnet-...`, where the version precedes the family and
/// only a date suffix follows it).
fn claude_version(model: &str) -> Option<(u32, u32)> {
    for family in ["opus", "sonnet", "haiku"] {
        if let Some(idx) = model.find(family) {
            let rest = model[idx + family.len()..].trim_start_matches('-');
            let mut parts = rest.split(['-', '@']);
            let major = parts.next()?.parse::<u32>().ok()?;
            let minor = match parts.next() {
                Some(m) => m.parse::<u32>().ok()?,
                // New naming scheme (e.g. `claude-sonnet-5`) has no minor
                // segment; treat it as `.0`. Legacy names put the version
                // before the family and leave only an 8-digit date suffix
                // after it (e.g. `claude-3-5-sonnet-20241022`), so reject
                // implausibly large majors to keep returning `None` for those.
                None => {
                    if major >= 1000 {
                        return None;
                    }
                    0
                }
            };
            return Some((major, minor));
        }
    }
    None
}

/// Whether `model`'s parsed version is at least `major.minor`. Returns `false`
/// when the version cannot be parsed (e.g. legacy or non-Claude names).
fn version_at_least(model: &str, major: u32, minor: u32) -> bool {
    claude_version(model).is_some_and(|(maj, min)| maj > major || (maj == major && min >= minor))
}

/// Whether the model belongs to the Fable family (Claude Fable 5 /
/// Claude Mythos 5). These models use a new naming scheme without a minor
/// version segment (e.g. `claude-fable-5`), so they are detected by family
/// name instead of via `claude_version`. Both share the same API behavior:
/// thinking is always on (adaptive only), sampling parameters are removed,
/// and effort supports the full low..=max range including xhigh.
fn is_fable_family(model: &str) -> bool {
    model.contains("fable") || model.contains("mythos")
}

/// Whether the model rejects sampling parameters (`temperature`, `top_p`,
/// `top_k`) with a 400 error. True for the Fable family, Opus 4.7+, and
/// Sonnet 5+.
fn sampling_params_removed(model: &str) -> bool {
    is_fable_family(model)
        || (model.contains("opus") && version_at_least(model, 4, 7))
        || (model.contains("sonnet") && version_at_least(model, 5, 0))
}

// ---------------------------------------------------------------------------
// max_tokens defaults by model
// ---------------------------------------------------------------------------

/// Get the default max_tokens for a given model name.
fn default_max_tokens(model: &str) -> u32 {
    let has = |s: &str| model.contains(s);
    if is_fable_family(model)
        || (has("opus") && version_at_least(model, 4, 6))
        || (has("sonnet") && version_at_least(model, 5, 0))
    {
        128_000
    } else if (has("opus") && version_at_least(model, 4, 5))
        || (has("sonnet") && version_at_least(model, 4, 5))
        || (has("haiku") && version_at_least(model, 4, 5))
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

        let is_other_agent = matches!(&msg.role, ChatRole::Assistant)
            && msg
                .name
                .as_ref()
                .is_some_and(|name| name != self_agent_name);

        // Raw content-block replay (current agent only): when we captured the
        // assistant's original ordered blocks, replay them verbatim. This
        // preserves the exact thinking ↔ server_tool_use ↔
        // web_search_tool_result ↔ text interleaving, which the flattened
        // fields cannot reconstruct, so the Anthropic protocol's "latest
        // assistant turn thinking must be complete and unmodified" rule holds.
        // The raw blocks already contain any tool_use, so this supersedes the
        // tool_calls branch below. Other agents fall through to summary/drop.
        if msg.role == ChatRole::Assistant && !is_other_agent && !msg.raw_content_blocks.is_empty()
        {
            flush_pending_anthropic(&mut pending, &mut result);
            result.push(serde_json::json!({
                "role": "assistant",
                "content": serde_json::Value::Array(msg.raw_content_blocks.clone()),
            }));
            continue;
        }

        // Assistant messages with tool_calls: Anthropic uses tool_use content blocks.
        if let (ChatRole::Assistant, Some(tcs)) = (&msg.role, &msg.tool_calls) {
            flush_pending_anthropic(&mut pending, &mut result);

            let mut content_blocks: Vec<serde_json::Value> = Vec::new();
            if !is_other_agent {
                content_blocks.extend(thinking_blocks_to_json(&msg.thinking_blocks));
            }
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

        // Fallthrough for assistant turns WITHOUT captured `raw_content_blocks`:
        // older sessions saved before raw-block capture, or turns whose blocks
        // `prune` cleared. Turns that DO carry raw blocks were already replayed
        // verbatim by the branch above (thinking included, in original order),
        // so this path is only reached when the ordered block sequence is
        // unavailable.
        //
        // For these, intentionally do NOT replay thinking blocks. Per the
        // Anthropic protocol thinking is only required while a tool-use loop is
        // still open (handled above, where `tool_calls` is set); for a finished
        // turn it is optional. Without the raw blocks we cannot reconstruct the
        // original order — the turn may have interleaved `server_tool_use` /
        // `web_search_tool_result` blocks between thinking blocks (e.g. web
        // search) — so emitting the thinking blocks back-to-back would reorder
        // the sequence and the API rejects it ("thinking blocks ... cannot be
        // modified"). So such messages fall through to regular-text handling.

        // Regular messages.
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

/// Serialize a slice of `ThinkingBlock`s into Anthropic content-block JSON.
///
/// `Thinking` becomes `{"type":"thinking","thinking":...,"signature":...}` and
/// `Redacted` becomes `{"type":"redacted_thinking","data":...}`. Order is
/// preserved so the caller can prepend the result directly to a content array.
fn thinking_blocks_to_json(blocks: &[ThinkingBlock]) -> Vec<serde_json::Value> {
    blocks
        .iter()
        .map(|block| match block {
            ThinkingBlock::Thinking { text, signature } => serde_json::json!({
                "type": BLOCK_TYPE_THINKING,
                "thinking": text,
                "signature": signature,
            }),
            ThinkingBlock::Redacted { data } => serde_json::json!({
                "type": BLOCK_TYPE_REDACTED_THINKING,
                "data": data,
            }),
        })
        .collect()
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

    // Fable family and Opus 4.7+ reject temperature/top_p/top_k with a 400,
    // so never send them for those models.
    if sampling_params_removed(model) {
        if sampling.temperature.is_some() || sampling.top_p.is_some() || sampling.top_k.is_some() {
            tracing::warn!(
                "Anthropic: model {model} does not accept temperature/top_p/top_k; \
                 ignoring configured sampling parameters"
            );
        }
    } else {
        // Temperature: clamp to 0-1 for Anthropic.
        if let Some(t) = sampling.temperature {
            let clamped = if enable_thinking {
                // When thinking is enabled, temperature must be 1.0.
                if (t - 1.0).abs() > f64::EPSILON {
                    tracing::warn!(
                        "Anthropic: thinking enabled, overriding temperature {t} to 1.0"
                    );
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

/// Check if a model supports adaptive thinking (Fable family, Opus/Sonnet 4.6
/// and later — including the new-scheme `claude-sonnet-5`).
fn supports_adaptive(model: &str) -> bool {
    is_fable_family(model)
        || ((model.contains("opus") || model.contains("sonnet")) && version_at_least(model, 4, 6))
}

/// Check if a model supports the effort parameter (adaptive models plus Opus 4.5).
fn supports_effort(model: &str) -> bool {
    supports_adaptive(model) || (model.contains("opus") && version_at_least(model, 4, 5))
}

/// Check if a model supports effort = "max" (any adaptive-capable model).
fn supports_max_effort(model: &str) -> bool {
    supports_adaptive(model)
}

/// Check if a model supports effort = "xhigh" (Fable family, Opus 4.7+, and
/// Sonnet 5+).
fn supports_xhigh_effort(model: &str) -> bool {
    is_fable_family(model)
        || (model.contains("opus") && version_at_least(model, 4, 7))
        || (model.contains("sonnet") && version_at_least(model, 5, 0))
}

/// Build the thinking parameter for the request body.
pub(crate) fn build_thinking_params(
    enable_thinking: bool,
    thinking_effort: Option<ThinkingEffort>,
    model: &str,
) -> Option<serde_json::Value> {
    if !enable_thinking {
        // Sonnet 5+ runs adaptive thinking by default when the `thinking` key
        // is omitted, so disabling requires an explicit `disabled` config.
        // The Fable family rejects `disabled` (thinking is always on there)
        // and older models default to off, so both omit the key entirely.
        if model.contains("sonnet") && version_at_least(model, 5, 0) {
            return Some(serde_json::json!({"type": "disabled"}));
        }
        return None;
    }

    if supports_adaptive(model) {
        // Fable family / Opus 4.6+ / Sonnet 4.6+: use adaptive thinking with
        // summarized display so the model emits thinking summary blocks the
        // TUI can render. Note: the Fable family rejects any non-adaptive
        // thinking config (`disabled` and `budget_tokens` both return 400);
        // adaptive is valid there, and when thinking is not enabled we omit
        // the `thinking` key entirely (see early return above), which Fable
        // treats as always-on adaptive thinking.
        Some(serde_json::json!({
            "type": "adaptive",
            "display": "summarized",
        }))
    } else {
        // Older models: use enabled + budget_tokens.
        // Xhigh/Max map to same budget as High (32768).
        let budget = match thinking_effort {
            Some(ThinkingEffort::Low) => 1024,
            Some(ThinkingEffort::High | ThinkingEffort::Xhigh | ThinkingEffort::Max) => 32768,
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
    // The Fable family's thinking is always on regardless of enable_thinking,
    // so a configured effort must still be honored there.
    if !supports_effort(model) || (!enable_thinking && !is_fable_family(model)) {
        return None;
    }

    thinking_effort.map(|effort| {
        let effort_str = match effort {
            ThinkingEffort::Low => "low",
            ThinkingEffort::Medium => "medium",
            ThinkingEffort::High => "high",
            ThinkingEffort::Xhigh => {
                if supports_xhigh_effort(model) {
                    "xhigh"
                } else {
                    // Downgrade to high on models that don't support xhigh.
                    "high"
                }
            }
            ThinkingEffort::Max => {
                if supports_max_effort(model) {
                    "max"
                } else {
                    // Downgrade to high on models that don't support max.
                    "high"
                }
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
    /// Accumulated text for the current `thinking` content block.
    thinking_text: String,
    /// Signature string collected from `signature_delta` events for the
    /// current `thinking` block.
    thinking_signature: String,
    /// Opaque `data` field captured from a `redacted_thinking` content block.
    redacted_data: Option<String>,
    /// Current `server_tool_use` ID, captured so the raw block can be replayed.
    server_tool_id: String,
    /// Accumulated text for the current `text` content block. The streamed
    /// `TextDelta`s are not otherwise retained, so this rebuilds the raw block.
    text_buf: String,
    /// Citation objects accumulated from `citations_delta` events for the
    /// current `text` block (e.g. web_search source citations).
    text_citations: Vec<serde_json::Value>,
    /// A finalized raw content block awaiting emission. Set just before a
    /// semantic event (ToolCall / ServerToolDone / ThinkingBlockDone) returns,
    /// then drained as a `RawContentBlock` on the next poll so raw blocks
    /// preserve the provider's original stream order.
    pending_raw: Option<serde_json::Value>,
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

            // Drain a raw content block queued by the previous poll, before
            // consuming more SSE events, so RawContentBlock events keep the
            // provider's original block order.
            if let Some(raw) = state.pending_raw.take() {
                return Some((StreamEvent::RawContentBlock(raw), (sse_stream, state, done)));
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

                                    if block_type == BLOCK_TYPE_THINKING {
                                        state.thinking_text.clear();
                                        state.thinking_signature.clear();
                                        state.redacted_data = None;
                                    }

                                    if block_type == BLOCK_TYPE_REDACTED_THINKING {
                                        state.thinking_text.clear();
                                        state.thinking_signature.clear();
                                        state.redacted_data = block
                                            .get("data")
                                            .and_then(|d| d.as_str())
                                            .map(str::to_string);
                                    }

                                    // Server-side tool (e.g. web_search): emit start,
                                    // accumulate input JSON, emit done at content_block_stop.
                                    if block_type == "server_tool_use" {
                                        state.server_tool_id = block
                                            .get("id")
                                            .and_then(|i| i.as_str())
                                            .unwrap_or("")
                                            .to_string();
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

                                    // Server tool result (e.g. web_search_tool_result)
                                    // arrives complete at block_start with no deltas.
                                    // Capture the whole block verbatim (it carries the
                                    // encrypted_content the protocol requires echoed back)
                                    // and emit it as a raw block now; its
                                    // content_block_stop becomes a no-op.
                                    if block_type == "web_search_tool_result" {
                                        state.current_block_type = Some(block_type);
                                        return Some((
                                            StreamEvent::RawContentBlock(block.clone()),
                                            (sse_stream, state, done),
                                        ));
                                    }

                                    // Text block: reset the per-block accumulators used
                                    // to rebuild the raw block at content_block_stop.
                                    if block_type == "text" {
                                        state.text_buf.clear();
                                        state.text_citations.clear();
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
                                                state.text_buf.push_str(text);
                                                return Some((
                                                    StreamEvent::TextDelta(text.to_string()),
                                                    (sse_stream, state, done),
                                                ));
                                            }
                                        }
                                        DELTA_TYPE_THINKING => {
                                            if let Some(thinking) =
                                                delta.get("thinking").and_then(|t| t.as_str())
                                                && !thinking.is_empty()
                                            {
                                                state.thinking_text.push_str(thinking);
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
                                        DELTA_TYPE_SIGNATURE => {
                                            if let Some(sig) =
                                                delta.get("signature").and_then(|s| s.as_str())
                                            {
                                                state.thinking_signature.push_str(sig);
                                            }
                                        }
                                        "citations_delta" => {
                                            // Accumulate citation objects for the current
                                            // text block (e.g. web_search source refs).
                                            // Each delta carries one citation under
                                            // `citation`.
                                            if let Some(citation) = delta.get("citation") {
                                                state.text_citations.push(citation.clone());
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                continue;
                            }

                            "content_block_stop" => {
                                // Emit tool call if this was a tool_use block.
                                if state.current_block_type.as_deref() == Some("tool_use") {
                                    let input: serde_json::Value =
                                        serde_json::from_str(&state.tool_args)
                                            .unwrap_or_else(|_| serde_json::json!({}));
                                    let event = StreamEvent::ToolCall {
                                        id: state.tool_id.clone(),
                                        name: state.tool_name.clone(),
                                        arguments: state.tool_args.clone(),
                                        thought_signature: None,
                                    };
                                    state.pending_raw = Some(serde_json::json!({
                                        "type": "tool_use",
                                        "id": state.tool_id,
                                        "name": state.tool_name,
                                        "input": input,
                                    }));
                                    state.current_block_type = None;
                                    state.tool_id.clear();
                                    state.tool_name.clear();
                                    state.tool_args.clear();
                                    return Some((event, (sse_stream, state, done)));
                                }
                                // Emit server tool done with accumulated query.
                                if state.current_block_type.as_deref() == Some("server_tool_use") {
                                    let input: serde_json::Value =
                                        serde_json::from_str(&state.tool_args)
                                            .unwrap_or_else(|_| serde_json::json!({}));
                                    let query = input
                                        .get("query")
                                        .and_then(|q| q.as_str())
                                        .map(|s| s.to_string());
                                    let event = StreamEvent::ServerToolDone {
                                        name: state.tool_name.clone(),
                                        query,
                                    };
                                    state.pending_raw = Some(serde_json::json!({
                                        "type": "server_tool_use",
                                        "id": state.server_tool_id,
                                        "name": state.tool_name,
                                        "input": input,
                                    }));
                                    state.current_block_type = None;
                                    state.server_tool_id.clear();
                                    state.tool_name.clear();
                                    state.tool_args.clear();
                                    return Some((event, (sse_stream, state, done)));
                                }
                                if state.current_block_type.as_deref() == Some(BLOCK_TYPE_THINKING)
                                {
                                    let text = std::mem::take(&mut state.thinking_text);
                                    let signature = std::mem::take(&mut state.thinking_signature);
                                    state.current_block_type = None;
                                    state.redacted_data = None;
                                    // A thinking block without a signature is illegal
                                    // replay state — emitting it would persist a block
                                    // that the next request would reject with HTTP 400.
                                    // Treat as fatal so direct stream consumers cannot
                                    // observe a later `Done` after the error.
                                    if signature.is_empty() {
                                        done = true;
                                        return Some((
                                            StreamEvent::Error(
                                                "Anthropic thinking block missing signature".into(),
                                            ),
                                            (sse_stream, state, done),
                                        ));
                                    }
                                    state.pending_raw = Some(serde_json::json!({
                                        "type": BLOCK_TYPE_THINKING,
                                        "thinking": text.clone(),
                                        "signature": signature.clone(),
                                    }));
                                    return Some((
                                        StreamEvent::ThinkingBlockDone(ThinkingBlock::Thinking {
                                            text,
                                            signature,
                                        }),
                                        (sse_stream, state, done),
                                    ));
                                }
                                if state.current_block_type.as_deref()
                                    == Some(BLOCK_TYPE_REDACTED_THINKING)
                                {
                                    let data = state.redacted_data.take();
                                    state.thinking_text.clear();
                                    state.thinking_signature.clear();
                                    state.current_block_type = None;
                                    // Missing `data` means the upstream content_block_start
                                    // never carried the opaque payload — replaying an empty
                                    // redacted_thinking block would be rejected with HTTP 400.
                                    // Fatal: do not let later events surface a `Done` after this.
                                    let Some(data) = data else {
                                        done = true;
                                        return Some((
                                            StreamEvent::Error(
                                                "Anthropic redacted_thinking block missing data"
                                                    .into(),
                                            ),
                                            (sse_stream, state, done),
                                        ));
                                    };
                                    state.pending_raw = Some(serde_json::json!({
                                        "type": BLOCK_TYPE_REDACTED_THINKING,
                                        "data": data.clone(),
                                    }));
                                    return Some((
                                        StreamEvent::ThinkingBlockDone(ThinkingBlock::Redacted {
                                            data,
                                        }),
                                        (sse_stream, state, done),
                                    ));
                                }
                                // Text block finalized: rebuild the raw block from the
                                // accumulated text/citations and emit it directly (no
                                // preceding semantic event, so no pending_raw needed).
                                if state.current_block_type.as_deref() == Some("text") {
                                    let text = std::mem::take(&mut state.text_buf);
                                    let citations = std::mem::take(&mut state.text_citations);
                                    state.current_block_type = None;
                                    if text.is_empty() && citations.is_empty() {
                                        continue;
                                    }
                                    let mut raw = serde_json::json!({
                                        "type": "text",
                                        "text": text,
                                    });
                                    if !citations.is_empty() {
                                        raw["citations"] = serde_json::Value::Array(citations);
                                    }
                                    return Some((
                                        StreamEvent::RawContentBlock(raw),
                                        (sse_stream, state, done),
                                    ));
                                }
                                state.current_block_type = None;
                                continue;
                            }

                            "message_delta" => {
                                let Ok(v) = serde_json::from_str::<serde_json::Value>(&data) else {
                                    continue;
                                };
                                // Extract cumulative usage (output_tokens).
                                if let Some(usage) = v.get("usage") {
                                    state.output_tokens = usage
                                        .get("output_tokens")
                                        .and_then(|t| t.as_u64())
                                        .unwrap_or(0)
                                        as u32;
                                }
                                // Safety refusal (Fable family): HTTP 200 with
                                // stop_reason "refusal". Emit a Refusal event but do
                                // not terminate the stream — the subsequent
                                // message_stop still carries Done with billed usage.
                                // Branch only on stop_reason; stop_details may be null.
                                let stop_reason = v
                                    .get("delta")
                                    .and_then(|d| d.get("stop_reason"))
                                    .and_then(|s| s.as_str());
                                if stop_reason == Some("refusal") {
                                    let details = v.get("delta").and_then(|d| {
                                        d.get("stop_details").filter(|s| !s.is_null())
                                    });
                                    let category = details
                                        .and_then(|s| s.get("category"))
                                        .and_then(|c| c.as_str())
                                        .map(str::to_string);
                                    let explanation = details
                                        .and_then(|s| s.get("explanation"))
                                        .and_then(|e| e.as_str())
                                        .map(str::to_string);
                                    return Some((
                                        StreamEvent::Refusal {
                                            category,
                                            explanation,
                                        },
                                        (sse_stream, state, done),
                                    ));
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
    use crate::ToolCallInfo;
    use futures::StreamExt;
    use krew_config::SamplingConfig;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Spin up a one-shot TCP server that returns `response_body` as the
    /// HTTP body and returns the URL the client should hit. The server only
    /// handles a single request and then exits.
    async fn run_sse_server(response_body: &'static str) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buffer = Vec::new();
            loop {
                let mut chunk = [0u8; 1024];
                let n = socket.read(&mut chunk).await.unwrap_or(0);
                if n == 0 {
                    break;
                }
                buffer.extend_from_slice(&chunk[..n]);
                if buffer.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}/")
    }

    async fn collect_sse_events(sse: &'static str) -> Vec<StreamEvent> {
        let url = run_sse_server(sse).await;
        let response = reqwest::get(&url).await.unwrap();
        let mut stream = Box::pin(build_event_stream(response));
        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event);
        }
        events
    }

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
    }

    // ---- Thinking block aggregation tests (2.5) ----

    #[tokio::test]
    async fn sse_thinking_block_aggregates_text_and_signature() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"Step 1 \"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"and Step 2.\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig-xyz\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_delta\ndata: {\"usage\":{\"output_tokens\":3}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        let deltas: Vec<&String> = events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::ThinkingDelta(text) => Some(text),
                _ => None,
            })
            .collect();
        assert_eq!(deltas.len(), 2);
        assert_eq!(deltas[0], "Step 1 ");
        assert_eq!(deltas[1], "and Step 2.");

        let block = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::ThinkingBlockDone(block) => Some(block.clone()),
                _ => None,
            })
            .expect("ThinkingBlockDone must be emitted");
        assert_eq!(
            block,
            ThinkingBlock::Thinking {
                text: "Step 1 and Step 2.".to_string(),
                signature: "sig-xyz".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn sse_redacted_thinking_block_emits_redacted_variant_no_delta() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\",\"data\":\"opaque-blob\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_delta\ndata: {\"usage\":{\"output_tokens\":1}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::ThinkingDelta(_))),
            "redacted_thinking must not emit ThinkingDelta"
        );

        let block = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::ThinkingBlockDone(block) => Some(block.clone()),
                _ => None,
            })
            .expect("ThinkingBlockDone must be emitted");
        assert_eq!(
            block,
            ThinkingBlock::Redacted {
                data: "opaque-blob".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn sse_signature_delta_no_longer_ignored() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"think\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"captured-sig\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        let signature = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::ThinkingBlockDone(ThinkingBlock::Thinking { signature, .. }) => {
                    Some(signature.clone())
                }
                _ => None,
            })
            .expect("Thinking block with signature must be emitted");
        assert_eq!(signature, "captured-sig");
    }

    #[tokio::test]
    async fn sse_interrupted_thinking_block_does_not_emit_done() {
        // Stream ends before content_block_stop arrives.
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"partial\"}}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::ThinkingBlockDone(_))),
            "incomplete thinking block must not emit ThinkingBlockDone"
        );
    }

    #[tokio::test]
    async fn sse_thinking_block_without_signature_emits_error_no_done() {
        // content_block_stop arrives but no signature_delta was ever seen —
        // emitting a block with an empty signature would be replayed verbatim
        // and rejected by Anthropic with HTTP 400. The Error must also be
        // terminal so downstream consumers don't observe a later `Done`.
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"reasoning\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::ThinkingBlockDone(_))),
            "thinking block without signature must not emit ThinkingBlockDone"
        );
        let err_idx = events
            .iter()
            .position(|e| matches!(e, StreamEvent::Error(msg) if msg.contains("missing signature")))
            .expect("must surface a missing-signature error");
        assert!(
            !events[err_idx + 1..]
                .iter()
                .any(|e| matches!(e, StreamEvent::Done(_))),
            "no Done event may follow a fatal malformed-thinking error"
        );
    }

    #[tokio::test]
    async fn sse_redacted_thinking_block_without_data_emits_error_no_done() {
        // redacted_thinking block_start arrives without `data`. The block is
        // unusable for replay; the parser must surface an Error rather than
        // fabricate an empty Redacted block. The Error must also be terminal.
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"redacted_thinking\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::ThinkingBlockDone(_))),
            "redacted_thinking without data must not emit ThinkingBlockDone"
        );
        let err_idx = events
            .iter()
            .position(|e| matches!(e, StreamEvent::Error(msg) if msg.contains("missing data")))
            .expect("must surface a missing-data error");
        assert!(
            !events[err_idx + 1..]
                .iter()
                .any(|e| matches!(e, StreamEvent::Done(_))),
            "no Done event may follow a fatal malformed-redacted error"
        );
    }

    // ---- convert_messages thinking-block tests (3.3) ----

    fn assistant_with_thinking_and_tool_call(
        agent_name: &str,
        blocks: Vec<ThinkingBlock>,
    ) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: "I'll use a tool.".to_string(),
            name: Some(agent_name.to_string()),
            tool_calls: Some(vec![ToolCallInfo {
                id: "tool_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a.rs"}"#.to_string(),
                thought_signature: None,
            }]),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: blocks,
            raw_content_blocks: Vec::new(),
        }
    }

    fn assistant_thinking_only(
        agent_name: &str,
        content: &str,
        blocks: Vec<ThinkingBlock>,
    ) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: content.to_string(),
            name: Some(agent_name.to_string()),
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: blocks,
            raw_content_blocks: Vec::new(),
        }
    }

    #[test]
    fn convert_assistant_with_thinking_and_tool_use() {
        let msg = assistant_with_thinking_and_tool_call(
            "agent1",
            vec![ThinkingBlock::Thinking {
                text: "think first".to_string(),
                signature: "sig-abc".to_string(),
            }],
        );
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        let blocks = result.messages[0]["content"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "thinking");
        assert_eq!(blocks[0]["thinking"], "think first");
        assert_eq!(blocks[0]["signature"], "sig-abc");
        assert_eq!(blocks[1]["type"], "text");
        assert_eq!(blocks[2]["type"], "tool_use");
        assert_eq!(blocks[2]["id"], "tool_1");
    }

    #[test]
    fn convert_assistant_with_redacted_thinking() {
        let msg = assistant_with_thinking_and_tool_call(
            "agent1",
            vec![ThinkingBlock::Redacted {
                data: "opaque".to_string(),
            }],
        );
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        let first = &result.messages[0]["content"][0];
        assert_eq!(first["type"], "redacted_thinking");
        assert_eq!(first["data"], "opaque");
        assert!(first.get("thinking").is_none());
        assert!(first.get("signature").is_none());
    }

    #[test]
    fn convert_other_agent_thinking_dropped() {
        let msg = assistant_with_thinking_and_tool_call(
            "other-agent",
            vec![ThinkingBlock::Thinking {
                text: "secret".to_string(),
                signature: "sig".to_string(),
            }],
        );
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::Assistant);
        let blocks = result.messages[0]["content"].as_array().unwrap();
        for block in blocks {
            assert_ne!(block["type"], "thinking");
            assert_ne!(block["type"], "redacted_thinking");
        }
    }

    #[test]
    fn convert_assistant_thinking_only_no_tool_use() {
        // A completed (end_turn) assistant turn has no pending tool_calls, so its
        // thinking blocks are optional and MUST be dropped on replay: we cannot
        // reproduce the exact original block sequence (e.g. interleaved
        // server_tool_use/web_search_tool_result), and any mismatch is rejected
        // by the API ("thinking blocks ... cannot be modified").
        let msg = assistant_thinking_only(
            "agent1",
            "final answer",
            vec![ThinkingBlock::Thinking {
                text: "deliberation".to_string(),
                signature: "sig-end".to_string(),
            }],
        );
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0]["role"], "assistant");
        // Only the final text remains; no thinking block is replayed.
        assert_eq!(result.messages[0]["content"], "final answer");
    }

    // ---- Raw content block capture / replay tests ----

    fn raw_blocks(events: &[StreamEvent]) -> Vec<serde_json::Value> {
        events
            .iter()
            .filter_map(|e| match e {
                StreamEvent::RawContentBlock(v) => Some(v.clone()),
                _ => None,
            })
            .collect()
    }

    #[tokio::test]
    async fn sse_thinking_block_emits_raw_content_block() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"reason\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"sig-1\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        let raws = raw_blocks(&events);
        assert_eq!(raws.len(), 1);
        assert_eq!(raws[0]["type"], "thinking");
        assert_eq!(raws[0]["thinking"], "reason");
        assert_eq!(raws[0]["signature"], "sig-1");
    }

    #[tokio::test]
    async fn sse_server_tool_use_emits_raw_with_id() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"srvtoolu_1\",\"name\":\"web_search\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\\\"rust\\\"}\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        let raw = raw_blocks(&events)
            .into_iter()
            .find(|v| v["type"] == "server_tool_use")
            .expect("must emit server_tool_use raw block");
        assert_eq!(raw["id"], "srvtoolu_1");
        assert_eq!(raw["name"], "web_search");
        assert_eq!(raw["input"]["query"], "rust");
    }

    #[tokio::test]
    async fn sse_web_search_tool_result_emits_raw_verbatim() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"web_search_tool_result\",\"tool_use_id\":\"srvtoolu_1\",\"content\":[{\"type\":\"web_search_result\",\"url\":\"https://e.com\",\"title\":\"E\",\"encrypted_content\":\"ENC123\"}]}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        let raw = raw_blocks(&events)
            .into_iter()
            .find(|v| v["type"] == "web_search_tool_result")
            .expect("must emit web_search_tool_result raw block");
        assert_eq!(raw["tool_use_id"], "srvtoolu_1");
        assert_eq!(raw["content"][0]["url"], "https://e.com");
        // encrypted_content must survive verbatim for protocol-compliant replay.
        assert_eq!(raw["content"][0]["encrypted_content"], "ENC123");
    }

    #[tokio::test]
    async fn sse_text_block_emits_raw_with_citations() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Answer\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"citations_delta\",\"citation\":{\"type\":\"web_search_result_location\",\"url\":\"https://e.com\",\"title\":\"E\"}}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        let raw = raw_blocks(&events)
            .into_iter()
            .find(|v| v["type"] == "text")
            .expect("must emit text raw block");
        assert_eq!(raw["text"], "Answer");
        assert_eq!(raw["citations"][0]["url"], "https://e.com");
    }

    #[tokio::test]
    async fn sse_interleaved_blocks_preserve_raw_order() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"t1\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"s1\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: content_block_start\ndata: {\"index\":1,\"content_block\":{\"type\":\"server_tool_use\",\"id\":\"srv1\",\"name\":\"web_search\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"query\\\":\\\"q\\\"}\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":1}\n\n\
                   event: content_block_start\ndata: {\"index\":2,\"content_block\":{\"type\":\"web_search_tool_result\",\"tool_use_id\":\"srv1\",\"content\":[{\"type\":\"web_search_result\",\"encrypted_content\":\"E\"}]}}\n\n\
                   event: content_block_stop\ndata: {\"index\":2}\n\n\
                   event: content_block_start\ndata: {\"index\":3,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":3,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"t2\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":3,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"s2\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":3}\n\n\
                   event: content_block_start\ndata: {\"index\":4,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":4,\"delta\":{\"type\":\"text_delta\",\"text\":\"done\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":4}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        let raws = raw_blocks(&events);
        let types: Vec<&str> = raws.iter().map(|v| v["type"].as_str().unwrap()).collect();
        assert_eq!(
            types,
            vec![
                "thinking",
                "server_tool_use",
                "web_search_tool_result",
                "thinking",
                "text",
            ]
        );
        // The two thinking blocks keep their original order and content.
        assert_eq!(raws[0]["thinking"], "t1");
        assert_eq!(raws[3]["thinking"], "t2");
    }

    #[tokio::test]
    async fn sse_thinking_without_signature_emits_no_raw_block() {
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"x\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;
        // An unsigned thinking block is fatal and must not leak a raw block.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::RawContentBlock(v) if v["type"] == "thinking"))
        );
        assert!(events.iter().any(|e| matches!(e, StreamEvent::Error(_))));
        assert!(!events.iter().any(|e| matches!(e, StreamEvent::Done(_))));
    }

    #[test]
    fn convert_replays_raw_content_blocks_verbatim() {
        let mut msg = assistant_thinking_only("agent1", "done", vec![]);
        msg.raw_content_blocks = vec![
            serde_json::json!({"type":"thinking","thinking":"t1","signature":"s1"}),
            serde_json::json!({"type":"server_tool_use","id":"srv1","name":"web_search","input":{"query":"q"}}),
            serde_json::json!({"type":"web_search_tool_result","tool_use_id":"srv1","content":[{"encrypted_content":"E"}]}),
            serde_json::json!({"type":"thinking","thinking":"t2","signature":"s2"}),
            serde_json::json!({"type":"text","text":"done"}),
        ];
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0]["role"], "assistant");
        let content = result.messages[0]["content"].as_array().unwrap();
        let types: Vec<&str> = content
            .iter()
            .map(|b| b["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            types,
            vec![
                "thinking",
                "server_tool_use",
                "web_search_tool_result",
                "thinking",
                "text",
            ]
        );
        // encrypted_content is replayed unchanged.
        assert_eq!(content[2]["content"][0]["encrypted_content"], "E");
    }

    #[test]
    fn convert_raw_blocks_supersede_tool_calls() {
        // A message carrying BOTH tool_calls and captured raw blocks must replay
        // only the raw blocks (which already embed the tool_use), never both.
        let mut msg = assistant_with_thinking_and_tool_call("agent1", vec![]);
        msg.raw_content_blocks = vec![
            serde_json::json!({"type":"text","text":"hi"}),
            serde_json::json!({"type":"tool_use","id":"tool_1","name":"read_file","input":{"path":"a.rs"}}),
        ];
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        let content = result.messages[0]["content"].as_array().unwrap();
        assert_eq!(content.len(), 2);
        let tool_uses = content.iter().filter(|b| b["type"] == "tool_use").count();
        assert_eq!(tool_uses, 1);
    }

    #[test]
    fn convert_other_agent_raw_blocks_not_replayed() {
        let mut msg = assistant_thinking_only("other-agent", "hello", vec![]);
        msg.raw_content_blocks = vec![
            serde_json::json!({"type":"thinking","thinking":"secret","signature":"s"}),
            serde_json::json!({"type":"text","text":"hello"}),
        ];
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::Assistant);
        // Another agent's raw blocks are never replayed; it falls back to the
        // prefixed-text summary, so no thinking leaks across agents.
        let content = &result.messages[0]["content"];
        assert!(
            content.is_string(),
            "expected string content, got {content:?}"
        );
        let text = content.as_str().unwrap();
        assert!(text.contains("hello"));
        assert!(!text.contains("secret"));
    }

    #[test]
    fn convert_empty_raw_blocks_uses_field_reconstruction() {
        // No raw blocks captured → fall back to thinking_blocks + tool_calls.
        let msg = assistant_with_thinking_and_tool_call(
            "agent1",
            vec![ThinkingBlock::Thinking {
                text: "t".to_string(),
                signature: "s".to_string(),
            }],
        );
        assert!(msg.raw_content_blocks.is_empty());
        let result = convert_messages(&[msg], "agent1", &OtherAgentRole::User);
        let blocks = result.messages[0]["content"].as_array().unwrap();
        assert_eq!(blocks[0]["type"], "thinking");
        assert_eq!(blocks.last().unwrap()["type"], "tool_use");
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
    fn thinking_opus_4_8_adaptive() {
        let result = build_thinking_params(true, None, "claude-opus-4-8");
        let val = result.unwrap();
        assert_eq!(val["type"], "adaptive");
        assert_eq!(val["display"], "summarized");
        assert!(val.get("budget_tokens").is_none());
    }

    #[test]
    fn thinking_opus_4_8_with_effort() {
        let thinking = build_thinking_params(true, Some(ThinkingEffort::High), "claude-opus-4-8");
        assert_eq!(thinking.unwrap()["type"], "adaptive");

        let output = build_output_config(true, Some(ThinkingEffort::High), "claude-opus-4-8");
        assert_eq!(output.unwrap()["effort"], "high");
    }

    #[test]
    fn thinking_opus_4_8_max_effort() {
        let output = build_output_config(true, Some(ThinkingEffort::Max), "claude-opus-4-8");
        assert_eq!(output.unwrap()["effort"], "max");
    }

    #[test]
    fn max_tokens_opus_4_8() {
        assert_eq!(default_max_tokens("claude-opus-4-8"), 128_000);
    }

    #[test]
    fn thinking_future_versions_use_adaptive() {
        // Versions beyond 4.8 (e.g. 4.9, 4.10, 5.0) must auto-resolve to
        // adaptive without further code changes.
        for model in [
            "claude-opus-4-9",
            "claude-opus-4-10",
            "claude-sonnet-4-9",
            "claude-opus-5-0",
        ] {
            let val = build_thinking_params(true, None, model).unwrap();
            assert_eq!(val["type"], "adaptive", "{model} should use adaptive");
        }
        // Future Opus keeps the 128k default; future Sonnet stays at 64k.
        assert_eq!(default_max_tokens("claude-opus-4-9"), 128_000);
        assert_eq!(default_max_tokens("claude-opus-5-0"), 128_000);
        assert_eq!(default_max_tokens("claude-sonnet-4-9"), 64_000);
    }

    #[test]
    fn version_parsing_handles_suffixes_and_legacy() {
        assert_eq!(claude_version("claude-opus-4-8"), Some((4, 8)));
        assert_eq!(claude_version("claude-opus-4-8-20260301"), Some((4, 8)));
        // Vertex form with an `@date` suffix.
        assert_eq!(claude_version("claude-opus-4-8@20260301"), Some((4, 8)));
        assert_eq!(claude_version("claude-opus-4-10"), Some((4, 10)));
        assert_eq!(claude_version("claude-opus-5-0"), Some((5, 0)));
        // New naming scheme without a minor segment → parsed as `.0`.
        assert_eq!(claude_version("claude-sonnet-5"), Some((5, 0)));
        assert_eq!(
            claude_version("claude-sonnet-5-20260630"),
            Some((5, 20260630))
        );
        assert_eq!(
            claude_version("claude-sonnet-5@20260630"),
            Some((5, 20260630))
        );
        // Legacy ordering (family after the version) is not parsed → None.
        assert_eq!(claude_version("claude-3-5-sonnet-20241022"), None);
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
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
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
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        };
        let converted = convert_messages(&[msg], "agent", &OtherAgentRole::User);
        let tool_result = &converted.messages[0]["content"][0];
        assert_eq!(tool_result["type"], "tool_result");
        // content should be a plain string, not an array
        assert!(tool_result["content"].is_string());
        assert_eq!(tool_result["content"], "file content here");
    }

    // ---- End-to-end thinking + tool_use replay test (8.1) ----

    struct CapturedRequest {
        body: serde_json::Value,
    }

    /// Run a server that handles two sequential POSTs. The first reply is
    /// `first_sse`; the second reply is `second_sse`. Returns the body the
    /// client sent on the second POST.
    async fn run_two_request_server(
        first_sse: &'static str,
        second_sse: &'static str,
    ) -> (String, tokio::task::JoinHandle<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            // First request: discard body, send first_sse.
            let (mut socket, _) = listener.accept().await.unwrap();
            consume_http_request(&mut socket).await;
            write_http_response(&mut socket, first_sse).await;
            drop(socket);

            // Second request: capture body, send second_sse.
            let (mut socket, _) = listener.accept().await.unwrap();
            let body = consume_http_request(&mut socket).await;
            write_http_response(&mut socket, second_sse).await;
            CapturedRequest {
                body: serde_json::from_slice(&body).unwrap(),
            }
        });
        (format!("http://{addr}"), handle)
    }

    async fn consume_http_request(socket: &mut tokio::net::TcpStream) -> Vec<u8> {
        let mut buffer = Vec::new();
        let mut content_length: usize = 0;
        loop {
            let mut chunk = [0u8; 1024];
            let n = socket.read(&mut chunk).await.unwrap_or(0);
            if n == 0 {
                break;
            }
            buffer.extend_from_slice(&chunk[..n]);
            let request = String::from_utf8_lossy(&buffer);
            if let Some(header_end) = request.find("\r\n\r\n") {
                content_length = request[..header_end]
                    .lines()
                    .find_map(|line| {
                        let (name, value) = line.split_once(':')?;
                        name.eq_ignore_ascii_case("content-length")
                            .then(|| value.trim().parse::<usize>().unwrap())
                    })
                    .unwrap_or(0);
                let body_len = buffer.len() - header_end - 4;
                if body_len >= content_length {
                    break;
                }
            }
        }
        let header_end = String::from_utf8_lossy(&buffer).find("\r\n\r\n").unwrap();
        let body_start = header_end + 4;
        buffer[body_start..body_start + content_length].to_vec()
    }

    async fn write_http_response(socket: &mut tokio::net::TcpStream, body: &str) {
        let response = format!(
            "HTTP/1.1 200 OK\r\ncontent-type: text/event-stream\r\ncontent-length: {}\r\n\r\n{}",
            body.len(),
            body
        );
        socket.write_all(response.as_bytes()).await.unwrap();
    }

    #[tokio::test]
    async fn end_to_end_thinking_block_replayed_on_second_turn() {
        // First SSE: thinking + tool_use.
        let first_sse = "event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":5}}}\n\n\
                         event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                         event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"thinking_delta\",\"thinking\":\"I will read the file\"}}\n\n\
                         event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"e2e-sig\"}}\n\n\
                         event: content_block_stop\ndata: {\"index\":0}\n\n\
                         event: content_block_start\ndata: {\"index\":1,\"content_block\":{\"type\":\"tool_use\",\"id\":\"toolu_1\",\"name\":\"read_file\"}}\n\n\
                         event: content_block_delta\ndata: {\"index\":1,\"delta\":{\"type\":\"input_json_delta\",\"partial_json\":\"{\\\"path\\\":\\\"a.rs\\\"}\"}}\n\n\
                         event: content_block_stop\ndata: {\"index\":1}\n\n\
                         event: message_delta\ndata: {\"usage\":{\"output_tokens\":3}}\n\n\
                         event: message_stop\ndata: {}\n\n";
        let second_sse = "event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_two_request_server(first_sse, second_sse).await;

        let config = LlmClientConfig {
            agent_name: "claude".to_string(),
            model: "claude-opus-4-7".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            other_agent_role: OtherAgentRole::User,
            retry_config: krew_config::RetryConfig::default(),
            enable_thinking: true,
            thinking_effort: None,
            enable_web_search: false,
            extra_headers: Vec::new(),
        };
        let client = AnthropicClient::new(config);

        // First request: drive the stream and aggregate thinking + tool_call.
        let user = ChatMessage::text(ChatRole::User, "read it", None);
        let mut stream = client
            .chat_stream(
                std::slice::from_ref(&user),
                &[],
                &SamplingConfig::default(),
                None,
            )
            .await
            .unwrap();
        let mut aggregated = ThinkingBlock::Redacted {
            data: String::new(),
        };
        let mut tool_id = String::new();
        let mut tool_name = String::new();
        let mut tool_args = String::new();
        while let Some(event) = stream.next().await {
            match event {
                StreamEvent::ThinkingBlockDone(b) => aggregated = b,
                StreamEvent::ToolCall {
                    id,
                    name,
                    arguments,
                    ..
                } => {
                    tool_id = id;
                    tool_name = name;
                    tool_args = arguments;
                }
                _ => {}
            }
        }
        let (text, signature) = match aggregated {
            ThinkingBlock::Thinking { text, signature } => (text, signature),
            _ => panic!("first turn must aggregate a Thinking variant"),
        };
        assert_eq!(signature, "e2e-sig");

        // Build next-turn history: user + assistant(thinking + tool_use) + tool_result.
        let assistant = ChatMessage {
            role: ChatRole::Assistant,
            content: String::new(),
            name: Some("claude".to_string()),
            tool_calls: Some(vec![ToolCallInfo {
                id: tool_id.clone(),
                name: tool_name,
                arguments: tool_args,
                thought_signature: None,
            }]),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: vec![ThinkingBlock::Thinking { text, signature }],
            raw_content_blocks: Vec::new(),
        };
        let tool_result = ChatMessage {
            role: ChatRole::Tool,
            content: "fn main() {}".to_string(),
            name: Some("read_file".to_string()),
            tool_calls: None,
            tool_call_id: Some(tool_id),
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: Vec::new(),
            raw_content_blocks: Vec::new(),
        };

        let _second_stream = client
            .chat_stream(
                &[user, assistant, tool_result],
                &[],
                &SamplingConfig::default(),
                None,
            )
            .await
            .unwrap();
        let captured = handle.await.unwrap();

        let messages = captured.body["messages"].as_array().unwrap();
        let assistant_msg = &messages[1];
        let content = assistant_msg["content"].as_array().unwrap();
        assert_eq!(content[0]["type"], "thinking");
        assert_eq!(content[0]["signature"], "e2e-sig");
        assert_eq!(content[0]["thinking"], "I will read the file");
    }

    // ---- Fable family detection & capability tests ----

    #[test]
    fn fable_family_detection() {
        assert!(is_fable_family("claude-fable-5"));
        assert!(is_fable_family("claude-mythos-5"));
        assert!(is_fable_family("claude-fable-5-20260601"));
        assert!(is_fable_family("claude-fable-5@20260601"));
        assert!(!is_fable_family("claude-opus-4-8"));
        assert!(!is_fable_family("claude-sonnet-4-6"));
    }

    #[test]
    fn fable_capabilities() {
        for model in ["claude-fable-5", "claude-mythos-5"] {
            assert!(supports_adaptive(model));
            assert!(supports_effort(model));
            assert!(supports_max_effort(model));
            assert!(supports_xhigh_effort(model));
            assert!(sampling_params_removed(model));
            assert_eq!(default_max_tokens(model), 128_000);
        }
    }

    #[test]
    fn xhigh_support_by_model() {
        assert!(supports_xhigh_effort("claude-opus-4-7"));
        assert!(supports_xhigh_effort("claude-opus-4-8"));
        assert!(supports_xhigh_effort("claude-sonnet-5"));
        assert!(!supports_xhigh_effort("claude-opus-4-6"));
        assert!(!supports_xhigh_effort("claude-sonnet-4-6"));
    }

    #[test]
    fn sampling_params_removed_by_model() {
        assert!(sampling_params_removed("claude-opus-4-7"));
        assert!(sampling_params_removed("claude-opus-4-8"));
        assert!(sampling_params_removed("claude-sonnet-5"));
        assert!(!sampling_params_removed("claude-opus-4-6"));
        assert!(!sampling_params_removed("claude-sonnet-4-6"));
    }

    #[test]
    fn sonnet_5_capabilities() {
        // Claude Sonnet 5 uses the new naming scheme without a minor segment;
        // it must be recognized as an adaptive, effort/xhigh/max-capable model
        // that rejects sampling parameters (like Opus 4.7+).
        let model = "claude-sonnet-5";
        assert!(supports_adaptive(model));
        assert!(supports_effort(model));
        assert!(supports_max_effort(model));
        assert!(supports_xhigh_effort(model));
        assert!(sampling_params_removed(model));
        // Sonnet 5 raises the output ceiling to 128k (its tokenizer also
        // produces ~30% more tokens, so the old 64k default truncates sooner).
        assert_eq!(default_max_tokens(model), 128_000);
        // Thinking uses adaptive (never budget_tokens, which would 400).
        let val = build_thinking_params(true, None, model).unwrap();
        assert_eq!(val["type"], "adaptive");
    }

    #[test]
    fn thinking_sonnet_5_disabled_sends_explicit_disabled() {
        // Omitting the `thinking` key on Sonnet 5 silently runs adaptive
        // thinking; disabling requires an explicit `disabled` config.
        for model in [
            "claude-sonnet-5",
            "claude-sonnet-5-20260630",
            "claude-sonnet-5@20260630",
        ] {
            let val = build_thinking_params(false, None, model).unwrap();
            assert_eq!(val["type"], "disabled", "{model}");
            assert!(val.get("budget_tokens").is_none(), "{model}");
        }
        // Fable still omits the key (explicit `disabled` returns 400 there),
        // and pre-5 models keep omitting it (off is their default).
        assert!(build_thinking_params(false, None, "claude-fable-5").is_none());
        assert!(build_thinking_params(false, None, "claude-mythos-5").is_none());
        assert!(build_thinking_params(false, None, "claude-sonnet-4-6").is_none());
        assert!(build_thinking_params(false, None, "claude-opus-4-8").is_none());
    }

    #[test]
    fn output_config_fable_effort_without_thinking_flag() {
        // Fable's thinking is always on server-side, so a configured effort
        // must be sent even when enable_thinking is false.
        let output = build_output_config(false, Some(ThinkingEffort::Low), "claude-fable-5");
        assert_eq!(output.unwrap()["effort"], "low");
        // Non-Fable models keep the old gating.
        assert!(build_output_config(false, Some(ThinkingEffort::Low), "claude-opus-4-8").is_none());
    }

    #[test]
    fn sampling_fable_omits_sampling_params() {
        let sampling = SamplingConfig {
            temperature: Some(0.5),
            top_p: Some(0.9),
            top_k: Some(40),
            stop_sequences: Some(vec!["STOP".into()]),
            ..Default::default()
        };
        for model in ["claude-fable-5", "claude-opus-4-7", "claude-opus-4-8"] {
            for enable_thinking in [false, true] {
                let params = build_sampling_params(&sampling, model, enable_thinking);
                assert!(!params.contains_key("temperature"), "{model}");
                assert!(!params.contains_key("top_p"), "{model}");
                assert!(!params.contains_key("top_k"), "{model}");
                // max_tokens and stop_sequences still allowed.
                assert!(params.contains_key("max_tokens"), "{model}");
                assert_eq!(params["stop_sequences"], serde_json::json!(["STOP"]));
            }
        }
    }

    #[test]
    fn thinking_fable_adaptive() {
        let result = build_thinking_params(true, None, "claude-fable-5");
        let val = result.unwrap();
        assert_eq!(val["type"], "adaptive");
        assert_eq!(val["display"], "summarized");
        assert!(val.get("budget_tokens").is_none());
    }

    #[test]
    fn thinking_fable_disabled_omits_param() {
        // Fable rejects an explicit `disabled` config; the thinking key must
        // be omitted entirely when thinking is not enabled.
        let result = build_thinking_params(false, Some(ThinkingEffort::High), "claude-fable-5");
        assert!(result.is_none());
    }

    #[test]
    fn output_config_fable_xhigh() {
        let output = build_output_config(true, Some(ThinkingEffort::Xhigh), "claude-fable-5");
        assert_eq!(output.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn output_config_fable_max() {
        let output = build_output_config(true, Some(ThinkingEffort::Max), "claude-fable-5");
        assert_eq!(output.unwrap()["effort"], "max");
    }

    #[test]
    fn output_config_opus_4_7_xhigh() {
        let output = build_output_config(true, Some(ThinkingEffort::Xhigh), "claude-opus-4-7");
        assert_eq!(output.unwrap()["effort"], "xhigh");
    }

    #[test]
    fn output_config_opus_4_6_xhigh_downgraded() {
        let output = build_output_config(true, Some(ThinkingEffort::Xhigh), "claude-opus-4-6");
        assert_eq!(output.unwrap()["effort"], "high");
    }

    // ---- Refusal stop_reason tests ----

    #[tokio::test]
    async fn sse_refusal_mid_stream_emits_refusal_then_done() {
        let sse = "event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":10}}}\n\n\
                   event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"text\",\"text\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"partial\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"category\":\"cyber\",\"explanation\":\"declined\"}},\"usage\":{\"output_tokens\":7}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        let refusal_idx = events
            .iter()
            .position(|e| {
                matches!(
                    e,
                    StreamEvent::Refusal {
                        category: Some(c),
                        explanation: Some(x),
                    } if c == "cyber" && x == "declined"
                )
            })
            .expect("must emit Refusal with category and explanation");
        // Done must still follow, carrying billed usage.
        let done = events[refusal_idx + 1..]
            .iter()
            .find_map(|e| match e {
                StreamEvent::Done(usage) => Some(usage.clone()),
                _ => None,
            })
            .expect("Done must follow Refusal");
        assert_eq!(done.completion_tokens, 7);
    }

    #[tokio::test]
    async fn sse_refusal_pre_output_empty_content() {
        // Pre-output refusal: no content blocks at all.
        let sse = "event: message_start\ndata: {\"message\":{\"usage\":{\"input_tokens\":10}}}\n\n\
                   event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":{\"category\":\"bio\"}},\"usage\":{\"output_tokens\":0}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(events.iter().any(|e| matches!(
            e,
            StreamEvent::Refusal {
                category: Some(c),
                explanation: None,
            } if c == "bio"
        )));
        assert!(events.iter().any(|e| matches!(e, StreamEvent::Done(_))));
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::TextDelta(_))),
            "pre-output refusal must not emit any text"
        );
    }

    #[tokio::test]
    async fn sse_refusal_null_stop_details() {
        // stop_details may be null; branch only on stop_reason.
        let sse = "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"refusal\",\"stop_details\":null},\"usage\":{\"output_tokens\":0}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(events.iter().any(|e| matches!(
            e,
            StreamEvent::Refusal {
                category: None,
                explanation: None,
            }
        )));
    }

    #[tokio::test]
    async fn sse_non_refusal_stop_reason_no_refusal_event() {
        let sse = "event: message_delta\ndata: {\"delta\":{\"stop_reason\":\"end_turn\"},\"usage\":{\"output_tokens\":5}}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events
                .iter()
                .any(|e| matches!(e, StreamEvent::Refusal { .. }))
        );
        assert!(events.iter().any(|e| matches!(e, StreamEvent::Done(_))));
    }

    // ---- Fable display:"omitted" empty thinking block ----

    #[tokio::test]
    async fn sse_empty_thinking_block_with_signature_ok() {
        // Fable with display:"omitted" emits thinking blocks whose text is an
        // empty string but which still carry a signature. They must round-trip
        // (ThinkingBlockDone + RawContentBlock) without an Error.
        let sse = "event: content_block_start\ndata: {\"index\":0,\"content_block\":{\"type\":\"thinking\",\"thinking\":\"\"}}\n\n\
                   event: content_block_delta\ndata: {\"index\":0,\"delta\":{\"type\":\"signature_delta\",\"signature\":\"omitted-sig\"}}\n\n\
                   event: content_block_stop\ndata: {\"index\":0}\n\n\
                   event: message_stop\ndata: {}\n\n";
        let events = collect_sse_events(sse).await;

        assert!(
            !events.iter().any(|e| matches!(e, StreamEvent::Error(_))),
            "empty-text thinking block with signature must not error"
        );
        let (text, signature) = events
            .iter()
            .find_map(|e| match e {
                StreamEvent::ThinkingBlockDone(ThinkingBlock::Thinking { text, signature }) => {
                    Some((text.clone(), signature.clone()))
                }
                _ => None,
            })
            .expect("must emit ThinkingBlockDone");
        assert_eq!(text, "");
        assert_eq!(signature, "omitted-sig");
        let raw = raw_blocks(&events);
        assert_eq!(raw[0]["type"], "thinking");
        assert_eq!(raw[0]["thinking"], "");
        assert_eq!(raw[0]["signature"], "omitted-sig");
    }

    // ---- End-to-end Fable request body ----

    /// One-shot server that captures the request body and replies with `sse`.
    async fn run_capture_server(
        sse: &'static str,
    ) -> (String, tokio::task::JoinHandle<CapturedRequest>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let body = consume_http_request(&mut socket).await;
            write_http_response(&mut socket, sse).await;
            CapturedRequest {
                body: serde_json::from_slice(&body).unwrap(),
            }
        });
        (format!("http://{addr}"), handle)
    }

    #[tokio::test]
    async fn end_to_end_fable_request_body() {
        let sse = "event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;

        let config = LlmClientConfig {
            agent_name: "claude".to_string(),
            model: "claude-fable-5".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            other_agent_role: OtherAgentRole::User,
            retry_config: krew_config::RetryConfig::default(),
            enable_thinking: true,
            thinking_effort: Some(ThinkingEffort::Xhigh),
            enable_web_search: false,
            extra_headers: Vec::new(),
        };
        let client = AnthropicClient::new(config);

        let sampling = SamplingConfig {
            temperature: Some(0.7),
            top_p: Some(0.9),
            top_k: Some(40),
            ..Default::default()
        };
        let user = ChatMessage::text(ChatRole::User, "hi", None);
        let mut stream = client
            .chat_stream(std::slice::from_ref(&user), &[], &sampling, None)
            .await
            .unwrap();
        while stream.next().await.is_some() {}
        let captured = handle.await.unwrap();
        let body = &captured.body;

        assert_eq!(body["model"], "claude-fable-5");
        // Sampling params must never be sent to Fable.
        assert!(body.get("temperature").is_none());
        assert!(body.get("top_p").is_none());
        assert!(body.get("top_k").is_none());
        assert_eq!(body["max_tokens"], 128_000);
        // Thinking must be adaptive without budget_tokens.
        assert_eq!(body["thinking"]["type"], "adaptive");
        assert!(body["thinking"].get("budget_tokens").is_none());
        assert_eq!(body["output_config"]["effort"], "xhigh");
    }

    #[tokio::test]
    async fn end_to_end_fable_no_thinking_omits_thinking_key() {
        let sse = "event: message_stop\ndata: {}\n\n";
        let (base_url, handle) = run_capture_server(sse).await;

        let config = LlmClientConfig {
            agent_name: "claude".to_string(),
            model: "claude-fable-5".to_string(),
            api_key: "test-key".to_string(),
            base_url: Some(base_url),
            other_agent_role: OtherAgentRole::User,
            retry_config: krew_config::RetryConfig::default(),
            enable_thinking: false,
            thinking_effort: None,
            enable_web_search: false,
            extra_headers: Vec::new(),
        };
        let client = AnthropicClient::new(config);

        let user = ChatMessage::text(ChatRole::User, "hi", None);
        let mut stream = client
            .chat_stream(
                std::slice::from_ref(&user),
                &[],
                &SamplingConfig::default(),
                None,
            )
            .await
            .unwrap();
        while stream.next().await.is_some() {}
        let captured = handle.await.unwrap();

        // Fable rejects `thinking: {type: "disabled"}`; the key must be absent.
        assert!(captured.body.get("thinking").is_none());
        assert!(captured.body.get("output_config").is_none());
    }
}
