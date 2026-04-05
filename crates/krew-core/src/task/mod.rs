//! Task engine — a thin wrapper around `run_agent_loop` that handles channel
//! management and result collection.
//!
//! Provides `run_task()` (sync await, ApprovalRequests auto-denied) and
//! `run_task_with_events()` (caller gets the event stream and handles
//! approvals).

mod types;

pub use types::{TaskRequest, TaskResult};

use std::future::Future;

use krew_llm::{ChatMessage, ChatRole, ServerToolUseInfo, Usage};
use tokio::sync::mpsc;

use crate::agent::{AgentLoopContext, run_agent_loop};
use crate::event::{AgentEvent, ReviewDecision};

/// Run a task synchronously: await completion and return the result.
///
/// An internal event consumer is spawned to collect `Done`/`Error` events.
/// If the caller's configuration causes an `ApprovalRequest`, the consumer
/// automatically replies with `ReviewDecision::Denied` to prevent deadlock.
pub async fn run_task(req: TaskRequest) -> TaskResult {
    run_task_inner(req, None).await
}

/// Run a task and return both a future and an event receiver.
///
/// The caller is responsible for consuming the event stream. If an
/// `ApprovalRequest` arrives, the caller MUST reply via the `respond`
/// oneshot. If the caller drops the receiver, the `respond` sender is
/// dropped and the agent loop receives `ReviewDecision::Denied`.
///
/// **WARNING**: If the caller holds the receiver but never reads
/// `ApprovalRequest` events, the task will deadlock — the approval
/// oneshot stays alive in the unbounded queue and the agent loop blocks
/// waiting for a response. Either consume all events or drop the receiver.
pub fn run_task_with_events(
    req: TaskRequest,
) -> (
    impl Future<Output = TaskResult>,
    mpsc::UnboundedReceiver<AgentEvent>,
) {
    let (external_tx, external_rx) = mpsc::unbounded_channel::<AgentEvent>();
    let future = run_task_inner(req, Some(external_tx));
    (future, external_rx)
}

/// Shared implementation for `run_task` and `run_task_with_events`.
///
/// When `event_tx` is `None` (run_task mode), `ApprovalRequest` events are
/// auto-denied and all other non-terminal events are dropped.
///
/// When `event_tx` is `Some` (run_task_with_events mode), all events
/// (including `ApprovalRequest`, `Done`, `Error`) are forwarded to the
/// caller. If forwarding an `ApprovalRequest` fails (external_rx dropped),
/// the consumer auto-denies to prevent deadlock.
async fn run_task_inner(
    req: TaskRequest,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
) -> TaskResult {
    let (tx, rx) = mpsc::unbounded_channel::<AgentEvent>();

    // Spawn consumer: collects result from Done/Error, forwards events
    // to the external channel (if provided), auto-denies approval
    // requests when no external consumer exists or when forwarding fails.
    let agent_name = req.agent_name.clone();
    let consumer = tokio::spawn(consume_events(rx, event_tx));

    // Build message list.
    let mut messages = Vec::new();
    if let Some(sp) = &req.system_prompt {
        messages.push(ChatMessage::text(ChatRole::System, sp.clone(), None));
    }
    messages.push(ChatMessage::text(ChatRole::User, req.prompt.clone(), None));

    // Keep a copy of initial messages for the complete history.
    let initial_messages = messages.clone();

    // Build on_retry that emits AgentEvent::Retrying through the channel,
    // so callers (and the forwarder) can observe retry attempts.
    let retry_tx = tx.clone();
    let on_retry = move |info: krew_llm::common::RetryInfo| {
        let _ = retry_tx.send(AgentEvent::Retrying {
            attempt: info.attempt,
            max_attempts: info.max_attempts,
            reason: info.reason.clone(),
            delay_secs: info.delay_secs,
        });
    };

    let ctx = AgentLoopContext {
        client: &req.client,
        tools: &req.tools,
        tool_defs: &req.tool_defs,
        sampling: &req.sampling,
        on_retry: &on_retry,
        tx: &tx,
        agent_name: &req.agent_name,
        max_rounds: req.max_rounds,
        approval_mode: req.approval_mode,
        approval_cache: &req.approval_cache,
        allow_rules: &req.allow_rules,
        deny_rules: &req.deny_rules,
        ask_rules: &req.ask_rules,
        cwd: &req.cwd,
        whisper_targets: None,
    };

    run_agent_loop(&ctx, &mut messages).await;

    // Drop tx to close the channel so the consumer finishes.
    drop(tx);

    let consumed = consumer.await.unwrap_or_else(|_| ConsumedResult {
        final_text: "task consumer panicked".to_string(),
        intermediate_messages: Vec::new(),
        server_tool_uses: Vec::new(),
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
        is_error: true,
    });

    // Build complete conversation history:
    // initial messages (system + user) + intermediate (tool rounds) + final assistant.
    let mut complete_messages = initial_messages;
    complete_messages.extend(consumed.intermediate_messages);
    if !consumed.is_error {
        complete_messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: consumed.final_text.clone(),
            name: Some(agent_name),
            tool_calls: None,
            tool_call_id: None,
            server_tool_uses: consumed.server_tool_uses,
            addressee: None,
            whisper_targets: None,
            created_at: chrono::Utc::now(),
            usage: Some(consumed.usage.clone()),
            images: Vec::new(),
        });
    }

    TaskResult {
        final_text: consumed.final_text,
        messages: complete_messages,
        usage: consumed.usage,
        is_error: consumed.is_error,
    }
}

/// Collected result from consuming the agent loop event stream.
struct ConsumedResult {
    final_text: String,
    intermediate_messages: Vec<ChatMessage>,
    server_tool_uses: Vec<ServerToolUseInfo>,
    usage: Usage,
    is_error: bool,
}

/// Consume events from the agent loop channel.
///
/// - Collects result data from `Done`/`Error` (terminal events).
/// - Forwards ALL events (including terminal) to `event_tx` when present.
/// - Auto-denies `ApprovalRequest` when `event_tx` is `None`, or when
///   forwarding to `event_tx` fails (receiver dropped).
async fn consume_events(
    mut rx: mpsc::UnboundedReceiver<AgentEvent>,
    event_tx: Option<mpsc::UnboundedSender<AgentEvent>>,
) -> ConsumedResult {
    let mut result = ConsumedResult {
        final_text: String::new(),
        intermediate_messages: Vec::new(),
        server_tool_uses: Vec::new(),
        usage: Usage {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
        },
        is_error: false,
    };

    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::Done {
                usage,
                intermediate_messages,
                final_text,
                server_tool_uses,
            } => {
                result.final_text = final_text.clone();
                result.intermediate_messages = intermediate_messages.clone();
                result.server_tool_uses = server_tool_uses.clone();
                result.usage = usage.clone();
                if let Some(ref etx) = event_tx {
                    let _ = etx.send(AgentEvent::Done {
                        usage,
                        intermediate_messages,
                        final_text,
                        server_tool_uses,
                    });
                }
                break;
            }
            AgentEvent::Error {
                message,
                intermediate_messages,
            } => {
                result.final_text = message.clone();
                result.intermediate_messages = intermediate_messages.clone();
                result.is_error = true;
                if let Some(ref etx) = event_tx {
                    let _ = etx.send(AgentEvent::Error {
                        message,
                        intermediate_messages,
                    });
                }
                break;
            }
            AgentEvent::ApprovalRequest {
                tool_name,
                arguments,
                allow_session_approval,
                reason,
                respond,
            } => {
                // Try to forward to external consumer. If no consumer exists
                // or forwarding fails (rx dropped), auto-deny to prevent
                // the agent loop from hanging.
                if let Some(ref etx) = event_tx {
                    if etx
                        .send(AgentEvent::ApprovalRequest {
                            tool_name,
                            arguments,
                            allow_session_approval,
                            reason,
                            respond,
                        })
                        .is_err()
                    {
                        // external_rx was dropped; can't recover the respond
                        // sender since it was moved into the failed send.
                        // The oneshot sender is now dropped, so the agent
                        // loop's receiver will get default Denied.
                    }
                } else {
                    let _ = respond.send(ReviewDecision::Denied);
                }
            }
            other => {
                if let Some(ref etx) = event_tx {
                    let _ = etx.send(other);
                }
            }
        }
    }

    result
}
