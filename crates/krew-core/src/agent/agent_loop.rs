use std::sync::Arc;

use futures::StreamExt;
use krew_config::ApprovalMode;
use krew_llm::{ChatMessage, ChatRole, StreamEvent, ToolCallInfo, ToolDefinition, Usage};
use krew_tools::ToolRegistry;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, ApprovalCache, ReviewDecision};

use super::approval::{ToolApproval, cache_session_approval, check_tool_approval};

/// Context for a single agent loop execution, grouping all shared references.
pub(super) struct AgentLoopContext<'a> {
    pub(super) client: &'a Arc<dyn krew_llm::LlmClient>,
    pub(super) tools: &'a ToolRegistry,
    pub(super) tool_defs: &'a [ToolDefinition],
    pub(super) sampling: &'a krew_config::SamplingConfig,
    pub(super) on_retry: &'a (dyn Fn(krew_llm::common::RetryInfo) + Send + Sync),
    pub(super) tx: &'a mpsc::UnboundedSender<AgentEvent>,
    pub(super) agent_name: &'a str,
    pub(super) max_rounds: u32,
    pub(super) approval_mode: ApprovalMode,
    pub(super) approval_cache: &'a ApprovalCache,
    pub(super) shell_allow_commands: &'a [String],
}

/// Run the agent's tool-call loop: stream LLM → execute tools → re-call LLM.
///
/// The loop exits when the LLM finishes without tool calls, when the
/// maximum number of tool rounds is reached, or on error.
pub(super) async fn run_agent_loop(ctx: &AgentLoopContext<'_>, messages: &mut Vec<ChatMessage>) {
    let mut total_usage = Usage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    };

    // Collect intermediate assistant+tool_calls and tool result messages
    // across all tool rounds, so they can be returned to the TUI for
    // persistence in the main session history.
    let mut tool_round_messages: Vec<ChatMessage> = Vec::new();

    for round in 0..=ctx.max_rounds {
        // Call the LLM.
        let stream = match ctx
            .client
            .chat_stream(messages, ctx.tool_defs, ctx.sampling, Some(ctx.on_retry))
            .await
        {
            Ok(s) => s,
            Err(e) => {
                let _ = ctx.tx.send(AgentEvent::Error {
                    message: e.to_string(),
                    intermediate_messages: std::mem::take(&mut tool_round_messages),
                });
                return;
            }
        };

        // Consume the stream, collecting text and tool calls.
        let result = consume_stream(stream, ctx.tx, ctx.agent_name).await;

        // Accumulate usage.
        if let Some(usage) = &result.usage {
            total_usage.prompt_tokens += usage.prompt_tokens;
            total_usage.completion_tokens += usage.completion_tokens;
            total_usage.total_tokens += usage.total_tokens;
        }

        // If there was a stream error, stop and report with collected messages.
        if let Some(error_msg) = result.error {
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
            });
            return;
        }

        // Safety check: max rounds exceeded.
        if round >= ctx.max_rounds {
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
        };
        tool_round_messages.push(assistant_msg.clone());
        messages.push(assistant_msg);

        // Split tool calls into three groups:
        // 1. readonly_calls: truly side-effect-free tools → parallel
        // 2. auto_write_calls: write/shell auto-approved (FullAuto, cache hit, allowlist) → serial
        // 3. approval_calls: need user approval → serial with prompt
        let mut readonly_calls = Vec::new();
        let mut auto_write_calls = Vec::new();
        let mut approval_calls: Vec<(&StreamToolCall, bool)> = Vec::new();
        for tc in &result.tool_calls {
            match check_tool_approval(
                &tc.name,
                &tc.arguments,
                ctx.tools,
                ctx.approval_mode,
                ctx.approval_cache,
                ctx.shell_allow_commands,
            )
            .await
            {
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
                } => approval_calls.push((tc, allow_session_approval)),
            }
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
                    arguments: args_str.clone(),
                });
                let tools_ref = &ctx.tools;
                let tx_ref = ctx.tx.clone();
                async move {
                    let args: serde_json::Value =
                        serde_json::from_str(&args_str).unwrap_or_default();
                    let handle = create_tool_context(&name, &tx_ref);
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
                arguments: args_str.clone(),
            });
            let args: serde_json::Value = serde_json::from_str(&args_str).unwrap_or_default();
            let handle = create_tool_context(&name, ctx.tx);
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

        for (tc, allow_session_approval) in &approval_calls {
            let name = tc.name.clone();
            let args_str = tc.arguments.clone();
            let id = tc.id.clone();

            let _ = ctx.tx.send(AgentEvent::ToolCallStart {
                name: name.clone(),
                arguments: args_str.clone(),
            });

            // Send approval request and block until user responds.
            let (resp_tx, resp_rx) = tokio::sync::oneshot::channel();
            let _ = ctx.tx.send(AgentEvent::ApprovalRequest {
                tool_name: name.clone(),
                arguments: args_str.clone(),
                allow_session_approval: *allow_session_approval,
                respond: resp_tx,
            });

            let decision = resp_rx.await.unwrap_or_default();

            match decision {
                ReviewDecision::Approved => {
                    let args: serde_json::Value =
                        serde_json::from_str(&args_str).unwrap_or_default();
                    let handle = create_tool_context(&name, ctx.tx);
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
                    let handle = create_tool_context(&name, ctx.tx);
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
            let summary = generate_tool_summary(&name, &result);
            let _ = ctx.tx.send(AgentEvent::ToolCallDone {
                name: name.clone(),
                result_summary: summary,
            });

            let tool_msg = ChatMessage {
                role: ChatRole::Tool,
                content: result.content,
                name: Some(name),
                tool_calls: None,
                tool_call_id: Some(id),
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
struct ToolContextHandle {
    ctx: krew_tools::ToolContext,
    /// Forwarder task that forwards streaming output to the TUI.
    /// Must be awaited after tool execution to ensure all output is delivered.
    forwarder: Option<tokio::task::JoinHandle<()>>,
}

/// Create a `ToolContext` for the given tool.
///
/// For shell tools, sets up an output channel that forwards each line
/// to the TUI as `AgentEvent::ToolCallOutput`. The returned handle
/// includes a forwarder task that must be awaited to drain output.
fn create_tool_context(
    tool_name: &str,
    tx: &mpsc::UnboundedSender<AgentEvent>,
) -> ToolContextHandle {
    if tool_name == "shell" {
        let (output_tx, mut output_rx) = mpsc::unbounded_channel::<String>();
        let event_tx = tx.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(text) = output_rx.recv().await {
                let _ = event_tx.send(AgentEvent::ToolCallOutput { text });
            }
        });
        ToolContextHandle {
            ctx: krew_tools::ToolContext {
                output_tx: Some(output_tx),
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
fn generate_tool_summary(tool_name: &str, result: &krew_tools::ToolResult) -> String {
    if result.is_error {
        return "error".to_string();
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
