//! `run_agent` tool implementation — delegates tasks to Sub-Agents in isolated contexts.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use krew_config::SamplingConfig;
use krew_llm::ToolDefinition;
use krew_tools::{ToolContext, ToolError, ToolHandler, ToolRegistry, ToolResult, ToolSpec};
use serde_json::{Value, json};
use tokio::sync::mpsc;

use super::types::SubAgentDef;
use crate::event::AgentEvent;
use crate::task::{TaskRequest, run_task_with_events};

/// Grouped permission-related configuration for Sub-Agent tool delegation.
pub struct PermissionConfig {
    pub approval_mode: krew_config::ApprovalMode,
    pub approval_cache: crate::event::ApprovalCache,
    pub allow_rules: Vec<krew_config::PermissionRule>,
    pub deny_rules: Vec<krew_config::PermissionRule>,
    pub ask_rules: Vec<krew_config::PermissionRule>,
    pub cwd: String,
}

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
    defs: std::collections::HashMap<String, SubAgentDef>,
    /// Depth guard: prevents nested Sub-Agent calls.
    is_running: Arc<AtomicBool>,
    /// Parent agent's LLM client.
    client: Arc<dyn krew_llm::LlmClient>,
    /// Parent agent's sampling configuration.
    sampling: SamplingConfig,
    /// Permission configuration.
    perms: PermissionConfig,
}

impl RunAgentTool {
    /// Create a new `RunAgentTool` with the given Sub-Agent definitions and parent resources.
    pub fn new(
        defs: Vec<SubAgentDef>,
        client: Arc<dyn krew_llm::LlmClient>,
        sampling: SamplingConfig,
        perms: PermissionConfig,
    ) -> Self {
        let defs_map = defs.into_iter().map(|d| (d.name.clone(), d)).collect();
        Self {
            defs: defs_map,
            is_running: Arc::new(AtomicBool::new(false)),
            client,
            sampling,
            perms,
        }
    }

    /// Set the `is_running` flag for testing the depth guard.
    #[cfg(test)]
    fn set_running_for_test(&self, running: bool) {
        self.is_running.store(running, Ordering::SeqCst);
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

        // Build TaskRequest with inherited parent permissions.
        let req = TaskRequest {
            prompt: task.to_string(),
            system_prompt: Some(def.system_prompt.clone()),
            client: Arc::clone(&self.client),
            tools,
            tool_defs,
            sampling: self.sampling.clone(),
            max_rounds: def.max_turns,
            agent_name: agent_name.to_string(),
            approval_mode: self.perms.approval_mode,
            approval_cache: self.perms.approval_cache.clone(),
            allow_rules: self.perms.allow_rules.clone(),
            deny_rules: self.perms.deny_rules.clone(),
            ask_rules: self.perms.ask_rules.clone(),
            cwd: self.perms.cwd.clone(),
        };

        let (task_fut, mut rx) = run_task_with_events(req);

        // Forwarder: forward progress events and approval requests to parent.
        // Result collection is handled by task_fut — the forwarder is purely
        // for side-effects (TUI display and approval interaction).
        let output_tx = ctx.output_tx.clone();
        let forwarder = tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                match event {
                    AgentEvent::ToolCallStart {
                        display_name,
                        arguments,
                        ..
                    } => {
                        if let Some(ref tx) = output_tx {
                            let args_summary = if arguments.chars().count() > 80 {
                                let truncated: String = arguments.chars().take(80).collect();
                                format!("{truncated}…")
                            } else {
                                arguments
                            };
                            let _ = tx.send(format!("🔧 {display_name}({args_summary})"));
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
                    AgentEvent::Retrying {
                        attempt,
                        max_attempts,
                        reason,
                        delay_secs,
                    } => {
                        if let Some(ref tx) = output_tx {
                            let _ = tx.send(format!(
                                "  ⟳ Retrying ({attempt}/{max_attempts}) in {delay_secs:.1}s: {reason}"
                            ));
                        }
                    }
                    AgentEvent::ApprovalRequest {
                        tool_name,
                        arguments,
                        allow_session_approval,
                        reason,
                        respond,
                    } => {
                        if let Some(ref ptx) = parent_tx {
                            let _ = ptx.send(AgentEvent::ApprovalRequest {
                                tool_name,
                                arguments,
                                allow_session_approval,
                                reason,
                                respond,
                            });
                        } else {
                            let _ = respond.send(crate::event::ReviewDecision::Denied);
                        }
                    }
                    _ => {}
                }
            }
        });

        // Await task completion — single source of truth for the result.
        let result = task_fut.await;

        // Wait for forwarder to finish draining events. Log if it panicked
        // — approval forwarding and TUI progress depend on this task.
        if let Err(e) = forwarder.await {
            tracing::error!("sub-agent event forwarder panicked: {e}");
        }

        let mut final_text = result.final_text;
        if final_text.is_empty() && !result.is_error {
            final_text = "(sub-agent produced no output)".to_string();
        }

        // Forward the final response text to parent output so the user
        // can see the sub-agent's answer.
        if !result.is_error
            && !final_text.is_empty()
            && let Some(ref tx) = ctx.output_tx
        {
            for line in final_text.lines() {
                let _ = tx.send(format!("  {line}"));
            }
        }

        Ok(ToolResult {
            content: final_text,
            is_error: result.is_error,
            images: vec![],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::pin::Pin;
    use std::sync::Mutex;

    use futures::stream;
    use krew_config::ApprovalMode;
    use krew_llm::{ChatMessage, StreamEvent, Usage};

    use crate::event::ApprovalCache;

    struct MockClient {
        response: String,
    }

    #[async_trait::async_trait]
    impl krew_llm::LlmClient for MockClient {
        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            _tools: &[krew_llm::ToolDefinition],
            _sampling: &krew_config::SamplingConfig,
            _on_retry: Option<&(dyn Fn(krew_llm::common::RetryInfo) + Send + Sync)>,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, krew_llm::LlmError>
        {
            let response = self.response.clone();
            let usage = Usage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            };
            Ok(Box::pin(stream::iter(vec![
                StreamEvent::TextDelta(response),
                StreamEvent::Done(usage),
            ])))
        }
    }

    /// Mock client that captures tool definitions it receives.
    struct ToolCapturingClient {
        captured_tools: Mutex<Option<Vec<String>>>,
    }

    #[async_trait::async_trait]
    impl krew_llm::LlmClient for ToolCapturingClient {
        async fn chat_stream(
            &self,
            _messages: &[ChatMessage],
            tools: &[krew_llm::ToolDefinition],
            _sampling: &krew_config::SamplingConfig,
            _on_retry: Option<&(dyn Fn(krew_llm::common::RetryInfo) + Send + Sync)>,
        ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>, krew_llm::LlmError>
        {
            let names: Vec<String> = tools.iter().map(|t| t.name.clone()).collect();
            *self.captured_tools.lock().unwrap() = Some(names);
            let usage = Usage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
            };
            Ok(Box::pin(stream::iter(vec![
                StreamEvent::TextDelta("done".to_string()),
                StreamEvent::Done(usage),
            ])))
        }
    }

    fn make_def() -> SubAgentDef {
        SubAgentDef {
            name: "helper".to_string(),
            description: "test agent".to_string(),
            system_prompt: "you are a test agent".to_string(),
            color: None,
            max_turns: 5,
            source_path: std::path::PathBuf::from("test.md"),
        }
    }

    fn make_perms() -> PermissionConfig {
        PermissionConfig {
            approval_mode: ApprovalMode::FullAuto,
            approval_cache: ApprovalCache::new(),
            allow_rules: vec![],
            deny_rules: vec![],
            ask_rules: vec![],
            cwd: ".".to_string(),
        }
    }

    fn make_ctx(registry: Arc<ToolRegistry>) -> ToolContext {
        ToolContext {
            output_tx: None,
            parent_event_tx: None,
            tool_registry: Some(Box::new(registry)),
        }
    }

    #[tokio::test]
    async fn depth_guard_prevents_nesting() {
        let client: Arc<dyn krew_llm::LlmClient> = Arc::new(MockClient {
            response: "hello".to_string(),
        });
        let tool = RunAgentTool::new(vec![make_def()], client, Default::default(), make_perms());
        tool.set_running_for_test(true);

        let ctx = make_ctx(Arc::new(ToolRegistry::empty()));
        let args = serde_json::json!({"agent": "helper", "task": "do something"});
        let result = ToolHandler::execute(&tool, args, &ctx).await.unwrap();

        assert!(result.is_error);
        assert!(result.content.contains("nesting is not allowed"));
    }

    #[tokio::test]
    async fn run_agent_excluded_from_tool_defs() {
        let capturing_client = Arc::new(ToolCapturingClient {
            captured_tools: Mutex::new(None),
        });
        let client: Arc<dyn krew_llm::LlmClient> = capturing_client.clone();
        let tool = RunAgentTool::new(vec![make_def()], client, Default::default(), make_perms());

        let mut registry = ToolRegistry::empty();
        let dummy = |name: &str| ToolSpec {
            name: name.to_string(),
            description: format!("{name} tool"),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        };
        registry.register(dummy("read_file"), Box::new(NoopHandler));
        registry.register(dummy("run_agent"), Box::new(NoopHandler));
        registry.register(dummy("grep"), Box::new(NoopHandler));

        let ctx = make_ctx(Arc::new(registry));
        let args = serde_json::json!({"agent": "helper", "task": "find files"});
        let result = ToolHandler::execute(&tool, args, &ctx).await.unwrap();
        assert!(!result.is_error);

        let captured = capturing_client
            .captured_tools
            .lock()
            .unwrap()
            .clone()
            .unwrap();
        assert!(captured.contains(&"read_file".to_string()));
        assert!(captured.contains(&"grep".to_string()));
        assert!(
            !captured.contains(&"run_agent".to_string()),
            "run_agent should be excluded"
        );
    }

    struct NoopHandler;

    #[async_trait::async_trait]
    impl ToolHandler for NoopHandler {
        fn name(&self) -> &str {
            "noop"
        }
        fn requires_approval(&self) -> bool {
            false
        }
        async fn execute(
            &self,
            _args: serde_json::Value,
            _ctx: &ToolContext,
        ) -> Result<ToolResult, ToolError> {
            Ok(ToolResult {
                content: String::new(),
                is_error: false,
                images: vec![],
            })
        }
    }
}
