//! `run_agent` tool implementation — delegates tasks to Sub-Agents in isolated contexts.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use krew_config::{ApprovalMode, SamplingConfig};
use krew_llm::{ChatMessage, ChatRole, ToolDefinition};
use krew_tools::{ToolContext, ToolError, ToolHandler, ToolRegistry, ToolResult, ToolSpec};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::types::SubAgentDef;
use crate::agent::{AgentLoopContext, run_agent_loop};
use crate::event::{AgentEvent, ApprovalCache};

/// Tool that runs a Sub-Agent in an isolated context.
///
/// The Sub-Agent shares the parent agent's runtime resources (client, tools,
/// approval cache, etc.) but operates on a completely independent message
/// history containing only `[system_prompt, user(task)]`.
///
/// The tool registry is passed at execution time via `ToolContext::tool_registry`
/// rather than stored here, avoiding circular references that would prevent
/// registration into the parent's `Arc<ToolRegistry>`.
pub struct RunAgentTool {
    /// Available Sub-Agent definitions keyed by name.
    defs: HashMap<String, SubAgentDef>,
    /// Depth guard: prevents nested Sub-Agent calls.
    is_running: Arc<AtomicBool>,
    /// Parent agent's LLM client.
    client: Arc<dyn krew_llm::LlmClient>,
    /// Parent agent's approval mode.
    approval_mode: ApprovalMode,
    /// Parent agent's session-scoped approval cache.
    approval_cache: ApprovalCache,
    /// Parent agent's sampling configuration.
    sampling: SamplingConfig,
    /// Shell commands auto-approved without user confirmation.
    shell_allow_commands: Vec<String>,
    /// Domains auto-approved for fetch_url.
    fetch_allow_domains: Vec<String>,
}

impl RunAgentTool {
    /// Create a new `RunAgentTool` with the given Sub-Agent definitions and parent resources.
    pub fn new(
        defs: Vec<SubAgentDef>,
        client: Arc<dyn krew_llm::LlmClient>,
        approval_mode: ApprovalMode,
        approval_cache: ApprovalCache,
        sampling: SamplingConfig,
        shell_allow_commands: Vec<String>,
        fetch_allow_domains: Vec<String>,
    ) -> Self {
        let defs_map = defs.into_iter().map(|d| (d.name.clone(), d)).collect();
        Self {
            defs: defs_map,
            is_running: Arc::new(AtomicBool::new(false)),
            client,
            approval_mode,
            approval_cache,
            sampling,
            shell_allow_commands,
            fetch_allow_domains,
        }
    }

    /// Build the tool specification with dynamic agent enum values.
    pub fn spec(&self) -> ToolSpec {
        let agent_names: Vec<Value> = self.defs.keys().map(|n| Value::String(n.clone())).collect();
        let agent_descriptions: Vec<String> = self
            .defs
            .values()
            .map(|d| format!("{}: {}", d.name, d.description))
            .collect();
        let agent_desc = agent_descriptions.join("; ");

        ToolSpec {
            name: "run_agent".to_string(),
            description: format!(
                "Delegate a task to a specialized Sub-Agent that runs in an isolated context. \
                 The Sub-Agent executes the task independently (with its own message history) \
                 and returns the final result. Use this for focused tasks where you want to \
                 avoid polluting the main conversation with intermediate tool calls. \
                 Available agents: {agent_desc}"
            ),
            parameters: json!({
                "type": "object",
                "required": ["agent", "task"],
                "properties": {
                    "agent": {
                        "type": "string",
                        "description": "Name of the Sub-Agent to invoke.",
                        "enum": agent_names,
                    },
                    "task": {
                        "type": "string",
                        "description": "Task description for the Sub-Agent.",
                    },
                },
            }),
        }
    }
}

#[async_trait::async_trait]
impl ToolHandler for RunAgentTool {
    fn name(&self) -> &str {
        "run_agent"
    }

    fn requires_approval(&self) -> bool {
        false
    }

    async fn execute(&self, args: Value, ctx: &ToolContext) -> Result<ToolResult, ToolError> {
        // Depth guard: prevent nested Sub-Agent calls.
        if self
            .is_running
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Ok(ToolResult {
                content: "Sub-agent nesting is not allowed".to_string(),
                is_error: true,
                images: vec![],
            });
        }

        // Guard to reset the flag on exit.
        struct ResetGuard(Arc<AtomicBool>);
        impl Drop for ResetGuard {
            fn drop(&mut self) {
                self.0.store(false, Ordering::SeqCst);
            }
        }
        let _guard = ResetGuard(self.is_running.clone());

        // Parse arguments.
        let agent_name = args
            .get("agent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'agent' parameter".into()))?;
        let task = args
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| ToolError::InvalidArgs("missing 'task' parameter".into()))?;

        // Get tool registry from context (passed by the agent loop).
        let tools = ctx
            .tool_registry
            .as_ref()
            .and_then(|boxed| boxed.downcast_ref::<Arc<ToolRegistry>>())
            .cloned()
            .ok_or_else(|| ToolError::Execution("tool registry not available in context".into()))?;

        // Look up the Sub-Agent definition.
        let def = match self.defs.get(agent_name) {
            Some(d) => d,
            None => {
                return Ok(ToolResult {
                    content: format!("Unknown sub-agent: {agent_name}"),
                    is_error: true,
                    images: vec![],
                });
            }
        };

        // Downcast parent_event_tx for approval forwarding.
        let parent_tx = ctx
            .parent_event_tx
            .as_ref()
            .and_then(|boxed| boxed.downcast_ref::<mpsc::UnboundedSender<AgentEvent>>())
            .cloned();

        // Build isolated message list.
        let mut messages = vec![
            ChatMessage::text(ChatRole::System, def.system_prompt.clone(), None),
            ChatMessage::text(ChatRole::User, task.to_string(), None),
        ];

        // Build tool_defs excluding `run_agent` to prevent nesting.
        let tool_defs: Vec<ToolDefinition> = tools
            .specs()
            .iter()
            .filter(|spec| spec.name != "run_agent")
            .map(|spec| ToolDefinition {
                name: spec.name.clone(),
                description: spec.description.clone(),
                parameters: spec.parameters.clone(),
            })
            .collect();

        // Create sub-agent event channel.
        let (sub_tx, mut sub_rx) = mpsc::unbounded_channel::<AgentEvent>();

        // Spawn event consumer BEFORE starting the loop, since the loop may
        // block on approval requests that the consumer needs to forward.
        let output_tx = ctx.output_tx.clone();
        let consumer_handle = tokio::spawn(async move {
            let mut final_text = String::new();
            let mut is_error = false;

            while let Some(event) = sub_rx.recv().await {
                match event {
                    AgentEvent::ToolCallStart { name, arguments } => {
                        if let Some(ref tx) = output_tx {
                            let args_summary = if arguments.len() > 80 {
                                format!("{}…", &arguments[..80])
                            } else {
                                arguments
                            };
                            let _ = tx.send(format!("🔧 {name}({args_summary})"));
                        }
                    }
                    AgentEvent::ToolCallOutput { text } => {
                        if let Some(ref tx) = output_tx {
                            let _ = tx.send(format!("  {text}"));
                        }
                    }
                    AgentEvent::ToolCallDone {
                        name: _,
                        result_summary,
                    } => {
                        if let Some(ref tx) = output_tx {
                            let _ = tx.send(format!("  ✓ {result_summary}"));
                        }
                    }
                    AgentEvent::ServerToolStart { name } => {
                        if let Some(ref tx) = output_tx {
                            let _ = tx.send(format!("🔍 {name}..."));
                        }
                    }
                    AgentEvent::ServerToolDone { name: _, query } => {
                        if let Some(ref tx) = output_tx
                            && let Some(q) = query
                        {
                            let _ = tx.send(format!("  ✓ {q}"));
                        }
                    }
                    AgentEvent::ApprovalRequest {
                        tool_name,
                        arguments,
                        allow_session_approval,
                        respond,
                    } => {
                        if let Some(ref ptx) = parent_tx {
                            let _ = ptx.send(AgentEvent::ApprovalRequest {
                                tool_name,
                                arguments,
                                allow_session_approval,
                                respond,
                            });
                        } else {
                            let _ = respond.send(crate::event::ReviewDecision::Denied);
                        }
                    }
                    AgentEvent::TextDelta(text) => {
                        final_text.push_str(&text);
                    }
                    AgentEvent::Done {
                        final_text: done_text,
                        ..
                    } => {
                        if final_text.is_empty() {
                            final_text = done_text;
                        }
                        // Forward the final response text to parent output so
                        // the user can see the sub-agent's answer.
                        if !final_text.is_empty()
                            && let Some(ref tx) = output_tx
                        {
                            for line in final_text.lines() {
                                let _ = tx.send(format!("  {line}"));
                            }
                        }
                        break;
                    }
                    AgentEvent::Error { message, .. } => {
                        final_text = message;
                        is_error = true;
                        break;
                    }
                    _ => {}
                }
            }

            (final_text, is_error)
        });

        // Build retry callback.
        let output_tx_for_retry = ctx.output_tx.clone();
        let on_retry = move |info: krew_llm::common::RetryInfo| {
            if let Some(ref tx) = output_tx_for_retry {
                let _ = tx.send(format!(
                    "  ⟳ Retrying ({}/{}) in {:.1}s: {}",
                    info.attempt, info.max_attempts, info.delay_secs, info.reason
                ));
            }
        };

        // Run the agent loop. This blocks until the sub-agent completes.
        // The loop sends events to sub_tx; the spawned consumer task processes them.
        let loop_ctx = AgentLoopContext {
            client: &self.client,
            tools: &tools,
            tool_defs: &tool_defs,
            sampling: &self.sampling,
            on_retry: &on_retry,
            tx: &sub_tx,
            agent_name,
            max_rounds: def.max_turns,
            approval_mode: self.approval_mode,
            approval_cache: &self.approval_cache,
            shell_allow_commands: &self.shell_allow_commands,
            fetch_allow_domains: &self.fetch_allow_domains,
            whisper_targets: None,
        };

        run_agent_loop(&loop_ctx, &mut messages).await;

        // Drop sub_tx to close the channel so the consumer finishes.
        drop(loop_ctx);
        drop(sub_tx);

        // Wait for the consumer to collect final results.
        let (mut final_text, is_error) = consumer_handle
            .await
            .unwrap_or_else(|_| ("sub-agent consumer task panicked".to_string(), true));

        if final_text.is_empty() && !is_error {
            final_text = "(sub-agent produced no output)".to_string();
        }

        Ok(ToolResult {
            content: final_text,
            is_error,
            images: vec![],
        })
    }
}
