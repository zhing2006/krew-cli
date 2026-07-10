use std::sync::Arc;

use futures::StreamExt;
use krew_config::ApprovalMode;
use krew_llm::{
    ChatMessage, ChatRole, StreamEvent, ThinkingBlock, ToolCallInfo, ToolDefinition, Usage,
};
use krew_tools::ToolRegistry;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, ApprovalCache, ReviewDecision};

use super::approval::{ApprovalContext, ToolApproval, cache_session_approval, check_tool_approval};

/// Context for a single agent loop execution, grouping all shared references.
pub(crate) struct AgentLoopContext<'a> {
    pub(crate) client: &'a Arc<dyn krew_llm::LlmClient>,
    pub(crate) tools: &'a Arc<ToolRegistry>,
    pub(crate) tool_defs: &'a [ToolDefinition],
    pub(crate) sampling: &'a krew_config::SamplingConfig,
    pub(crate) on_retry: &'a (dyn Fn(krew_llm::common::RetryInfo) + Send + Sync),
    pub(crate) tx: &'a mpsc::UnboundedSender<AgentEvent>,
    pub(crate) agent_name: &'a str,
    pub(crate) max_rounds: u32,
    pub(crate) approval_mode: ApprovalMode,
    pub(crate) approval_cache: &'a ApprovalCache,
    pub(crate) allow_rules: &'a [krew_config::PermissionRule],
    pub(crate) deny_rules: &'a [krew_config::PermissionRule],
    pub(crate) ask_rules: &'a [krew_config::PermissionRule],
    /// Working directory for path normalization in permission rules.
    pub(crate) cwd: &'a str,
    /// Whisper targets to stamp on all produced messages (None = not a whisper).
    pub(crate) whisper_targets: Option<Vec<String>>,
}

/// Run the agent's tool-call loop: stream LLM → execute tools → re-call LLM.
///
/// The loop exits when the LLM finishes without tool calls, when the
/// maximum number of tool rounds is reached, or on error.
pub(crate) async fn run_agent_loop(ctx: &AgentLoopContext<'_>, messages: &mut Vec<ChatMessage>) {
    let mut total_usage = Usage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };

    // Collect intermediate assistant+tool_calls and tool result messages
    // across all tool rounds, so they can be returned to the TUI for
    // persistence in the main session history.
    let mut tool_round_messages: Vec<ChatMessage> = Vec::new();
    let mut all_server_tool_uses: Vec<krew_llm::ServerToolUseInfo> = Vec::new();

    for round in 0..=ctx.max_rounds {
        // Call the LLM.
        let stream = match ctx
            .client
            .chat_stream(messages, ctx.tool_defs, ctx.sampling, Some(ctx.on_retry))
            .await
        {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(agent = ctx.agent_name, round, "LLM request failed: {e}",);
                let _ = ctx.tx.send(AgentEvent::Error {
                    message: e.to_string(),
                    intermediate_messages: std::mem::take(&mut tool_round_messages),
                });
                return;
            }
        };

        // Consume the stream, collecting text and tool calls.
        let result = consume_stream(stream, ctx.tx, ctx.agent_name).await;

        // Accumulate server tool uses.
        all_server_tool_uses.extend(result.server_tool_uses);

        // Accumulate usage.
        if let Some(usage) = &result.usage {
            total_usage.prompt_tokens += usage.prompt_tokens;
            total_usage.completion_tokens += usage.completion_tokens;
            total_usage.total_tokens += usage.total_tokens;
        }

        // If there was a stream error, stop and report with collected messages.
        if let Some(error_msg) = result.error {
            tracing::error!(
                agent = ctx.agent_name,
                round,
                "LLM stream error: {error_msg}",
            );
            let _ = ctx.tx.send(AgentEvent::Error {
                message: error_msg,
                intermediate_messages: tool_round_messages,
            });
            return;
        }

        // If no tool calls, we're done.
        if result.tool_calls.is_empty() {
            let _ = ctx.tx.send(AgentEvent::Done {
                usage: total_usage,
                intermediate_messages: tool_round_messages,
                final_text: result.text,
                server_tool_uses: all_server_tool_uses,
                final_thinking_blocks: result.thinking_blocks,
                final_raw_content_blocks: result.raw_content_blocks,
            });
            return;
        }

        // Safety check: max rounds exceeded.
        if round >= ctx.max_rounds {
            tracing::error!(
                agent = ctx.agent_name,
                "tool call loop exceeded maximum of {} rounds",
                ctx.max_rounds,
            );
            let _ = ctx.tx.send(AgentEvent::Error {
                message: format!(
                    "Tool call loop exceeded maximum of {} rounds",
                    ctx.max_rounds
                ),
                intermediate_messages: std::mem::take(&mut tool_round_messages),
            });
            return;
        }

        // Build the assistant message (with text + tool_calls).
        let assistant_msg = ChatMessage {
            role: ChatRole::Assistant,
            content: result.text,
            name: Some(ctx.agent_name.to_string()),
            tool_calls: Some(
                result
                    .tool_calls
                    .iter()
                    .map(|tc| ToolCallInfo {
                        id: tc.id.clone(),
                        name: tc.name.clone(),
                        arguments: tc.arguments.clone(),
                        thought_signature: tc.thought_signature.clone(),
                    })
                    .collect(),
            ),
            tool_call_id: None,
            server_tool_uses: Vec::new(),
            addressee: None,
            whisper_targets: ctx.whisper_targets.clone(),
            created_at: chrono::Utc::now(),
            usage: None,
            images: Vec::new(),
            thinking_blocks: result.thinking_blocks.clone(),
            raw_content_blocks: result.raw_content_blocks.clone(),
        };
        tool_round_messages.push(assistant_msg.clone());
        messages.push(assistant_msg);

        // Split tool calls into four groups:
        // 1. readonly_calls: truly side-effect-free tools → parallel
        // 2. auto_write_calls: write/shell auto-approved (FullAuto, cache hit, rules) → serial
        // 3. approval_calls: need user approval → serial with prompt
        // 4. denied_calls: denied by rule → immediate error result, no execution
        let mut readonly_calls = Vec::new();
        let mut auto_write_calls = Vec::new();
        let mut approval_calls: Vec<(&StreamToolCall, bool, Option<String>)> = Vec::new();
        let mut denied_calls: Vec<(&StreamToolCall, String)> = Vec::new();
        for tc in &result.tool_calls {
            let approval_ctx = ApprovalContext {
                tools: ctx.tools,
                mode: ctx.approval_mode,
                cache: ctx.approval_cache,
                allow_rules: ctx.allow_rules,
                deny_rules: ctx.deny_rules,
                ask_rules: ctx.ask_rules,
                cwd: ctx.cwd,
            };
            match check_tool_approval(&tc.name, &tc.arguments, &approval_ctx).await {
                ToolApproval::Auto => {
                    if ctx.tools.requires_approval(&tc.name) {
                        // Write tool auto-approved — execute serially to avoid races.
                        auto_write_calls.push(tc);
                    } else {
                        // Truly readonly — safe to execute in parallel.
                        readonly_calls.push(tc);
                    }
                }
                ToolApproval::NeedsApproval {
                    allow_session_approval,
                    reason,
                } => approval_calls.push((tc, allow_session_approval, reason)),
                ToolApproval::Denied { reason } => denied_calls.push((tc, reason)),
            }
        }

        // Phase 0: Process denied tool calls — immediate error results.
        for (tc, reason) in &denied_calls {
            let name = tc.name.clone();
            let args_str = tc.arguments.clone();
            let id = tc.id.clone();
            let _ = ctx.tx.send(AgentEvent::ToolCallStart {
                name: name.clone(),
                display_name: ctx.tools.display_name(&name),
                arguments: args_str.clone(),
            });
            let content = if reason.is_empty() {
                "Tool denied by rule.".to_string()
            } else {
                format!("Tool denied: {reason}")
            };
            let _ = ctx.tx.send(AgentEvent::ToolDenied {
                tool_name: name.clone(),
                reason: reason.clone(),
            });
            let tool_msg = ChatMessage {
                role: ChatRole::Tool,
                content: content.clone(),
                name: Some(name.clone()),
                tool_call_id: Some(id),
                tool_calls: None,
                images: Vec::new(),
                addressee: None,
                whisper_targets: None,
                created_at: chrono::Utc::now(),
                usage: None,
                server_tool_uses: Vec::new(),
                thinking_blocks: Vec::new(),
                raw_content_blocks: Vec::new(),
            };
            tool_round_messages.push(tool_msg.clone());
            messages.push(tool_msg);
        }

        // Phase 1: Execute readonly tools in parallel.
        let readonly_futures: Vec<_> = readonly_calls
            .iter()
            .map(|tc| {
                let name = tc.name.clone();
                let args_str = tc.arguments.clone();
                let id = tc.id.clone();
                let _ = ctx.tx.send(AgentEvent::ToolCallStart {
                    name: name.clone(),
                    display_name: ctx.tools.display_name(&name),
                    arguments: args_str.clone(),
                });
                let tools_ref = ctx.tools;
                let tx_ref = ctx.tx.clone();
                let tools_clone = Arc::clone(ctx.tools);
                async move {
                    let args: serde_json::Value =
                        serde_json::from_str(&args_str).unwrap_or_default();
                    let handle = create_tool_context(&name, &tx_ref, &tools_clone);
                    let result = tools_ref.dispatch(&name, args, &handle.ctx).await;
                    // Drop the sender so the forwarder sees channel closed.
                    drop(handle.ctx);
                    if let Some(f) = handle.forwarder {
                        let _ = f.await;
                    }
                    (id, name, args_str, result)
                }
            })
            .collect();

        let readonly_results = futures::future::join_all(readonly_futures).await;

        // Phase 2: Execute auto-approved write tools serially (avoid races on same file).
        let mut auto_write_results = Vec::new();
        for tc in &auto_write_calls {
            let name = tc.name.clone();
            let args_str = tc.arguments.clone();
            let id = tc.id.clone();
            let _ = ctx.tx.send(AgentEvent::ToolCallStart {
                name: name.clone(),
                display_name: ctx.tools.display_name(&name),
                arguments: args_str.clone(),
            });
            let args: serde_json::Value = serde_json::from_str(&args_str).unwrap_or_default();
            let handle = create_tool_context(&name, ctx.tx, ctx.tools);
            let result = ctx.tools.dispatch(&name, args, &handle.ctx).await;
            drop(handle.ctx);
            if let Some(f) = handle.forwarder {
                let _ = f.await;
            }
            auto_write_results.push((id, name, args_str, result));
        }

        // Phase 3: Execute approval-needed tools sequentially.
        let mut approval_results = Vec::new();
        let mut aborted = false;

        for (tc, allow_session_approval, ask_reason) in &approval_calls {
            let name = tc.name.clone();
            let args_str = tc.arguments.clone();
            let id = tc.id.clone();

            let _ = ctx.tx.send(AgentEvent::ToolCallStart {
                name: name.clone(),
                display_name: ctx.tools.display_name(&name),
                arguments: args_str.clone(),
            });

            // Send approval request and block until user responds.
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let _ = ctx.tx.send(AgentEvent::ApprovalRequest {
                tool_name: name.clone(),
                arguments: args_str.clone(),
                allow_session_approval: *allow_session_approval,
                reason: ask_reason.clone(),
                respond: resp_tx,
            });

            let decision = resp_rx.await.unwrap_or_default();

            match decision {
                ReviewDecision::Approved => {
                    let args: serde_json::Value =
                        serde_json::from_str(&args_str).unwrap_or_default();
                    let handle = create_tool_context(&name, ctx.tx, ctx.tools);
                    let result = ctx.tools.dispatch(&name, args, &handle.ctx).await;
                    drop(handle.ctx);
                    if let Some(f) = handle.forwarder {
                        let _ = f.await;
                    }
                    approval_results.push((id, name, args_str, result));
                }
                ReviewDecision::ApprovedForSession => {
                    // Cache approval keys for this tool.
                    cache_session_approval(&name, &args_str, ctx.approval_cache).await;
                    let args: serde_json::Value =
                        serde_json::from_str(&args_str).unwrap_or_default();
                    let handle = create_tool_context(&name, ctx.tx, ctx.tools);
                    let result = ctx.tools.dispatch(&name, args, &handle.ctx).await;
                    drop(handle.ctx);
                    if let Some(f) = handle.forwarder {
                        let _ = f.await;
                    }
                    approval_results.push((id, name, args_str, result));
                }
                ReviewDecision::Denied => {
                    let result = krew_tools::ToolResult {
                        content: format!("User denied execution of {name}."),
                        is_error: true,
                        images: vec![],
                    };
                    approval_results.push((id, name, args_str, result));
                }
                ReviewDecision::Abort => {
                    aborted = true;
                    break;
                }
            }
        }

        if aborted {
            tracing::info!(agent = ctx.agent_name, "user aborted the current operation");
            let _ = ctx.tx.send(AgentEvent::Error {
                message: "User aborted the current operation.".to_string(),
                intermediate_messages: std::mem::take(&mut tool_round_messages),
            });
            return;
        }

        // Combine all results and append to messages.
        let all_results = readonly_results
            .into_iter()
            .chain(auto_write_results)
            .chain(approval_results)
            .collect::<Vec<_>>();

        for (id, name, _args_str, result) in all_results {
            if result.is_error {
                let first_line = result.content.lines().next().unwrap_or("");
                tracing::warn!(
                    agent = ctx.agent_name,
                    tool = name.as_str(),
                    "tool returned error: {first_line}",
                );
            }
            // Forward MCP / fetch_url result content to TUI (displayed like shell output).
            let show_output = !result.is_error
                && !result.content.is_empty()
                && (krew_tools::mcp::is_mcp_tool(&name) || name == "fetch_url");
            if show_output {
                // Limit display to first N lines to keep the TUI readable.
                const MAX_DISPLAY_LINES: usize = 200;
                let total_lines = result.content.lines().count();
                for line in result.content.lines().take(MAX_DISPLAY_LINES) {
                    let _ = ctx.tx.send(AgentEvent::ToolCallOutput {
                        text: line.to_string(),
                    });
                }
                if total_lines > MAX_DISPLAY_LINES {
                    let _ = ctx.tx.send(AgentEvent::ToolCallOutput {
                        text: format!(
                            "... ({} more lines omitted)",
                            total_lines - MAX_DISPLAY_LINES
                        ),
                    });
                }
            }

            let summary = generate_tool_summary(&name, &result);
            let _ = ctx.tx.send(AgentEvent::ToolCallDone {
                name: name.clone(),
                result_summary: summary,
            });

            let images = result
                .images
                .into_iter()
                .map(|img| krew_llm::ImageContent {
                    data: img.data,
                    media_type: img.media_type,
                    filename: img.filename,
                })
                .collect();

            let tool_msg = ChatMessage {
                role: ChatRole::Tool,
                content: result.content,
                name: Some(name),
                tool_calls: None,
                tool_call_id: Some(id),
                server_tool_uses: Vec::new(),
                addressee: None,
                whisper_targets: ctx.whisper_targets.clone(),
                created_at: chrono::Utc::now(),
                usage: None,
                images,
                thinking_blocks: Vec::new(),
                raw_content_blocks: Vec::new(),
            };
            tool_round_messages.push(tool_msg.clone());
            messages.push(tool_msg);
        }
    }
}

/// Result of consuming a single LLM stream.
struct StreamResult {
    /// Accumulated text output.
    text: String,
    /// Tool calls received during the stream.
    tool_calls: Vec<StreamToolCall>,
    /// Server-side tool uses (e.g. web_search) for persistence.
    server_tool_uses: Vec<krew_llm::ServerToolUseInfo>,
    /// Thinking blocks aggregated from `StreamEvent::ThinkingBlockDone` in
    /// the order received. Replayed on the next turn so providers like
    /// Anthropic can validate the signatures.
    thinking_blocks: Vec<ThinkingBlock>,
    /// Raw, ordered content blocks aggregated from `StreamEvent::RawContentBlock`
    /// in stream order. Replayed verbatim on the next turn to preserve native
    /// content ordering and encrypted reasoning state.
    raw_content_blocks: Vec<serde_json::Value>,
    /// Token usage from the stream (if received).
    usage: Option<Usage>,
    /// Error message if the stream reported an error.
    error: Option<String>,
}

/// A tool call parsed from the stream.
struct StreamToolCall {
    id: String,
    name: String,
    arguments: String,
    thought_signature: Option<String>,
}

/// Consume an LLM stream, forwarding text events to the channel and
/// collecting tool calls.
async fn consume_stream(
    mut stream: std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>,
    tx: &mpsc::UnboundedSender<AgentEvent>,
    agent_name: &str,
) -> StreamResult {
    let mut result = StreamResult {
        text: String::new(),
        tool_calls: Vec::new(),
        server_tool_uses: Vec::new(),
        thinking_blocks: Vec::new(),
        raw_content_blocks: Vec::new(),
        usage: None,
        error: None,
    };

    while let Some(event) = stream.next().await {
        match event {
            StreamEvent::TextDelta(text) => {
                result.text.push_str(&text);
                if tx.send(AgentEvent::TextDelta(text)).is_err() {
                    return result;
                }
            }
            StreamEvent::ThinkingDelta(text) => {
                if tx.send(AgentEvent::ThinkingDelta(text)).is_err() {
                    return result;
                }
            }
            StreamEvent::ThinkingBlockDone(block) => {
                result.thinking_blocks.push(block);
            }
            StreamEvent::RawContentBlock(block) => {
                result.raw_content_blocks.push(block);
            }
            StreamEvent::ServerToolStart { name } => {
                if tx.send(AgentEvent::ServerToolStart { name }).is_err() {
                    return result;
                }
            }
            StreamEvent::ServerToolDone { name, query } => {
                result.server_tool_uses.push(krew_llm::ServerToolUseInfo {
                    name: name.clone(),
                    query: query.clone(),
                });
                if tx.send(AgentEvent::ServerToolDone { name, query }).is_err() {
                    return result;
                }
            }
            StreamEvent::ToolCall {
                id,
                name,
                arguments,
                thought_signature,
            } => {
                tracing::debug!(
                    agent = agent_name,
                    tool = name,
                    "Tool call received from LLM"
                );
                result.tool_calls.push(StreamToolCall {
                    id,
                    name,
                    arguments,
                    thought_signature,
                });
            }
            StreamEvent::Refusal {
                category,
                explanation,
            } => {
                // Safety refusal (Anthropic Fable family). The API contract
                // requires discarding any partially streamed output, and a
                // refused turn must not be replayed on the next request, so
                // drop everything accumulated so far. Keep consuming: a Done
                // event with billed usage still follows.
                result.text.clear();
                result.tool_calls.clear();
                result.thinking_blocks.clear();
                result.raw_content_blocks.clear();
                let mut msg = String::from("Model refused to respond (safety refusal");
                if let Some(c) = category {
                    msg.push_str(&format!(", category: {c}"));
                }
                msg.push(')');
                if let Some(e) = explanation {
                    msg.push_str(&format!(": {e}"));
                }
                result.error = Some(msg);
            }
            StreamEvent::Done(usage) => {
                result.usage = Some(usage);
                return result;
            }
            StreamEvent::Error(msg) => {
                result.error = Some(msg);
                return result;
            }
        }
    }

    result
}

/// Result of creating a tool context, including an optional forwarder handle
/// that must be awaited before sending `ToolCallDone`.
pub(crate) struct ToolContextHandle {
    pub(crate) ctx: krew_tools::ToolContext,
    /// Forwarder task that forwards streaming output to the TUI.
    /// Must be awaited after tool execution to ensure all output is delivered.
    pub(crate) forwarder: Option<tokio::task::JoinHandle<()>>,
}

/// Create a `ToolContext` for the given tool.
///
/// For shell and run_agent tools, sets up an output channel that forwards
/// each line to the TUI as `AgentEvent::ToolCallOutput`. For run_agent,
/// also sets `parent_event_tx` so Sub-Agent approval requests can be
/// forwarded to the parent agent's event channel.
pub(crate) fn create_tool_context(
    tool_name: &str,
    tx: &mpsc::UnboundedSender<AgentEvent>,
    tools: &Arc<krew_tools::ToolRegistry>,
) -> ToolContextHandle {
    if tool_name == "shell" || tool_name == "run_agent" {
        let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();
        let event_tx = tx.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(text) = output_rx.recv().await {
                let _ = event_tx.send(AgentEvent::ToolCallOutput { text });
            }
        });

        let parent_event_tx: Option<Box<dyn std::any::Any + Send + Sync>> =
            if tool_name == "run_agent" {
                Some(Box::new(tx.clone()))
            } else {
                None
            };

        let tool_registry: Option<Box<dyn std::any::Any + Send + Sync>> =
            if tool_name == "run_agent" {
                Some(Box::new(Arc::clone(tools)))
            } else {
                None
            };

        ToolContextHandle {
            ctx: krew_tools::ToolContext {
                output_tx: Some(output_tx),
                parent_event_tx,
                tool_registry,
            },
            forwarder: Some(forwarder),
        }
    } else {
        ToolContextHandle {
            ctx: krew_tools::ToolContext::default(),
            forwarder: None,
        }
    }
}

/// Generate a short summary string for TUI display of a tool result.
pub(crate) fn generate_tool_summary(tool_name: &str, result: &krew_tools::ToolResult) -> String {
    if result.is_error {
        // Show a concise error message instead of just "error".
        let msg = result.content.lines().next().unwrap_or("error");
        const MAX_ERROR_LEN: usize = 120;
        if msg.len() > MAX_ERROR_LEN {
            let boundary = msg.floor_char_boundary(MAX_ERROR_LEN);
            return format!("error: {}…", &msg[..boundary]);
        }
        return format!("error: {msg}");
    }

    // Extract the summary from the content's trailing "(N <unit>)" pattern.
    if let Some(summary) = result
        .content
        .rsplit_once('(')
        .and_then(|(_, rest)| rest.strip_suffix(')'))
    {
        return summary.to_string();
    }

    match tool_name {
        "read_file" => "done".to_string(),
        "glob" => "done".to_string(),
        "grep" => "done".to_string(),
        _ => "done".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::ApprovalCache;
    use futures::stream;
    use krew_config::{ApprovalMode, RetryConfig, SamplingConfig};
    use krew_llm::{LlmClient, LlmError, common::RetryInfo};
    use std::pin::Pin;
    use std::sync::Mutex;

    fn drain_events(rx: &mut mpsc::UnboundedReceiver<AgentEvent>) {
        while rx.try_recv().is_ok() {}
    }

    #[test]
    fn tool_summary_truncates_error_at_utf8_boundary() {
        let ascii_prefix = "a".repeat(118);
        let result = krew_tools::ToolResult {
            content: format!("{ascii_prefix}指tail"),
            is_error: true,
            images: vec![],
        };

        assert_eq!(
            generate_tool_summary("read_file", &result),
            format!("error: {ascii_prefix}…")
        );
    }

    #[test]
    fn tool_summary_keeps_short_unicode_error() {
        let result = krew_tools::ToolResult {
            content: "无法读取 MEMORY.md".to_string(),
            is_error: true,
            images: vec![],
        };

        assert_eq!(
            generate_tool_summary("read_file", &result),
            "error: 无法读取 MEMORY.md"
        );
    }

    #[tokio::test]
    async fn consume_stream_collects_thinking_blocks_in_order() {
        let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
        let block_a = ThinkingBlock::Thinking {
            text: "first".to_string(),
            signature: "sig-a".to_string(),
        };
        let block_b = ThinkingBlock::Redacted {
            data: "opaque".to_string(),
        };
        let events = vec![
            StreamEvent::ThinkingBlockDone(block_a.clone()),
            StreamEvent::ThinkingBlockDone(block_b.clone()),
            StreamEvent::TextDelta("hello".to_string()),
            StreamEvent::ToolCall {
                id: "tc_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a"}"#.to_string(),
                thought_signature: None,
            },
            StreamEvent::Done(Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            }),
        ];
        let stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> =
            Box::pin(stream::iter(events));
        let result = consume_stream(stream, &tx, "agent1").await;
        drain_events(&mut rx);

        assert_eq!(result.thinking_blocks, vec![block_a, block_b]);
        assert_eq!(result.text, "hello");
        assert_eq!(result.tool_calls.len(), 1);
    }

    #[tokio::test]
    async fn consume_stream_collects_raw_content_blocks_in_order() {
        let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
        let events = vec![
            StreamEvent::RawContentBlock(
                serde_json::json!({"type":"thinking","thinking":"t1","signature":"s1"}),
            ),
            StreamEvent::ThinkingBlockDone(ThinkingBlock::Thinking {
                text: "t1".to_string(),
                signature: "s1".to_string(),
            }),
            StreamEvent::RawContentBlock(serde_json::json!({"type":"text","text":"hi"})),
            StreamEvent::TextDelta("hi".to_string()),
            StreamEvent::Done(Usage::default()),
        ];
        let stream: Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>> =
            Box::pin(stream::iter(events));
        let result = consume_stream(stream, &tx, "agent1").await;
        drain_events(&mut rx);

        let types: Vec<&str> = result
            .raw_content_blocks
            .iter()
            .map(|v| v["type"].as_str().unwrap())
            .collect();
        assert_eq!(types, vec!["thinking", "text"]);
        assert_eq!(result.raw_content_blocks[0]["signature"], "s1");
    }

    /// LlmClient that returns canned event sequences per round and records
    /// the `messages` slice it was called with on each round so tests can
    /// inspect what the agent loop sent on follow-up rounds.
    struct CannedClient {
        rounds: Mutex<Vec<Vec<StreamEvent>>>,
        calls: Mutex<Vec<Vec<ChatMessage>>>,
    }

    #[async_trait::async_trait]
    impl LlmClient for CannedClient {
        async fn chat_stream(
            &self,
            messages: &[ChatMessage],
            _tools: &[krew_llm::ToolDefinition],
            _sampling: &SamplingConfig,
            _on_retry: Option<&(dyn Fn(RetryInfo) + Send + Sync)>,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, LlmError> {
            self.calls.lock().unwrap().push(messages.to_vec());
            let events = self.rounds.lock().unwrap().remove(0);
            Ok(Box::pin(stream::iter(events)))
        }
    }

    #[tokio::test]
    async fn agent_loop_attaches_thinking_blocks_to_each_round() {
        let block_round1 = ThinkingBlock::Thinking {
            text: "round1 reasoning".to_string(),
            signature: "sig-r1".to_string(),
        };
        let block_round2 = ThinkingBlock::Thinking {
            text: "round2 reasoning".to_string(),
            signature: "sig-r2".to_string(),
        };

        // Round 1: thinking + text + tool_call → agent loop iterates.
        // Round 2: thinking + text + Done (no tool_call) → loop exits.
        let round1 = vec![
            StreamEvent::ThinkingBlockDone(block_round1.clone()),
            StreamEvent::TextDelta("calling tool".to_string()),
            StreamEvent::ToolCall {
                id: "call_1".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"a"}"#.to_string(),
                thought_signature: None,
            },
            StreamEvent::Done(Usage::default()),
        ];
        let round2 = vec![
            StreamEvent::ThinkingBlockDone(block_round2.clone()),
            StreamEvent::TextDelta("final answer".to_string()),
            StreamEvent::Done(Usage::default()),
        ];
        let canned = Arc::new(CannedClient {
            rounds: Mutex::new(vec![round1, round2]),
            calls: Mutex::new(Vec::new()),
        });
        let client: Arc<dyn LlmClient> = canned.clone();

        let tools = Arc::new(krew_tools::ToolRegistry::empty());
        let tool_defs: Vec<krew_llm::ToolDefinition> = Vec::new();
        let sampling = SamplingConfig::default();
        let on_retry = |_: RetryInfo| {};
        let (tx, mut rx) = mpsc::unbounded_channel::<AgentEvent>();
        let approval_cache = ApprovalCache::new();
        let _ = RetryConfig::default();

        let ctx = AgentLoopContext {
            client: &client,
            tools: &tools,
            tool_defs: &tool_defs,
            sampling: &sampling,
            on_retry: &on_retry,
            tx: &tx,
            agent_name: "claude",
            max_rounds: 4,
            approval_mode: ApprovalMode::FullAuto,
            approval_cache: &approval_cache,
            allow_rules: &[],
            deny_rules: &[],
            ask_rules: &[],
            cwd: ".",
            whisper_targets: None,
        };
        let mut messages = vec![ChatMessage::text(krew_llm::ChatRole::User, "do it", None)];
        run_agent_loop(&ctx, &mut messages).await;
        drop(tx);

        let mut intermediate_round1 = None;
        let mut final_blocks = None;
        while let Some(event) = rx.recv().await {
            if let AgentEvent::Done {
                intermediate_messages,
                final_thinking_blocks,
                ..
            } = event
            {
                intermediate_round1 = intermediate_messages
                    .iter()
                    .find(|m| m.role == krew_llm::ChatRole::Assistant && m.tool_calls.is_some())
                    .map(|m| m.thinking_blocks.clone());
                final_blocks = Some(final_thinking_blocks);
                break;
            }
        }

        assert_eq!(intermediate_round1.unwrap(), vec![block_round1.clone()]);
        assert_eq!(final_blocks.unwrap(), vec![block_round2]);

        let history_round1 = messages
            .iter()
            .find(|m| m.role == krew_llm::ChatRole::Assistant && m.tool_calls.is_some())
            .expect("round1 assistant in messages history");
        assert_eq!(history_round1.thinking_blocks, vec![block_round1.clone()]);

        // Verify the agent loop replayed the round-1 thinking blocks on the
        // second LLM call. Without this, a regression where round 2 drops the
        // assistant's prior thinking_blocks would not be caught by the
        // final-history assertion alone.
        let calls = canned.calls.lock().unwrap();
        assert_eq!(calls.len(), 2, "expected exactly two LLM calls");
        let round2_messages = &calls[1];
        let round2_assistant = round2_messages
            .iter()
            .find(|m| m.role == krew_llm::ChatRole::Assistant && m.tool_calls.is_some())
            .expect("round2 input must contain the round1 assistant message");
        assert_eq!(
            round2_assistant.thinking_blocks,
            vec![block_round1],
            "round 2 must replay round 1's thinking_blocks back to the provider"
        );
    }
}
