use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use krew_config::{AgentConfig, ApiType, ApprovalMode, Config, OtherAgentRole, ProviderType};
use krew_llm::{
    AnthropicClient, ChatMessage, ChatRole, GoogleClient, LlmClient, LlmClientConfig,
    OpenAiChatClient, OpenAiResponsesClient, StreamEvent, ToolCallInfo, ToolDefinition, Usage,
};
use krew_tools::ToolRegistry;
use tokio::sync::mpsc;

use crate::event::{AgentEvent, ApprovalCache, ReviewDecision};

/// Default maximum number of tool-call loop rounds per agent turn.
const DEFAULT_MAX_TOOL_ROUNDS: u32 = 25;

/// Runtime state for a single agent in a session.
pub struct AgentRuntime {
    /// Agent configuration from settings.
    pub config: AgentConfig,
    /// LLM client for this agent's provider.
    pub client: Arc<dyn LlmClient>,
    /// Tools available to this agent.
    pub tools: Arc<ToolRegistry>,
    /// Whether the agent is currently generating a response.
    pub is_responding: bool,
    /// How to present other agents' messages in this agent's conversation.
    pub other_agent_role: OtherAgentRole,
    /// Tool approval policy for this session.
    pub approval_mode: ApprovalMode,
    /// Session-scoped approval cache (persists across agent turns).
    pub approval_cache: ApprovalCache,
    /// Shell commands that are auto-approved without user confirmation.
    pub shell_allow_commands: Vec<String>,
}

impl AgentRuntime {
    /// Start a streaming completion for this agent.
    ///
    /// Returns a channel receiver immediately. The HTTP request, stream
    /// consumption, and tool-call loop run in a spawned background task
    /// so the caller's event loop is never blocked.
    pub fn start_completion(
        &self,
        messages: Vec<ChatMessage>,
        project_instructions: Option<&str>,
        max_tool_rounds: Option<u32>,
    ) -> mpsc::UnboundedReceiver<AgentEvent> {
        let (tx, rx) = mpsc::unbounded_channel();

        // Send ResponseStart immediately so TUI can render the header.
        let _ = tx.send(AgentEvent::ResponseStart {
            agent_name: self.config.name.clone(),
            display_name: self.config.display_name.clone(),
            color: self.config.color.clone(),
        });

        // Build agent identity + optional custom system prompt.
        let other_agent_hint = "Their messages are prefixed with [agent_name] in the content.";
        let identity = format!(
            "You are {display_name}, powered by the {model} model.\n\
             Your agent name in this conversation is \"{name}\".\n\
             You are participating in a multi-agent conversation hosted by krew-cli.\n\
             Other agents in this conversation are DIFFERENT AI models, not you. \
             {other_agent_hint}\n\
             Respond as yourself — do not role-play or impersonate other agents.",
            display_name = self.config.display_name,
            model = self.config.model,
            name = self.config.name,
        );
        let agent_prompt = match &self.config.system_prompt {
            Some(prompt) if !prompt.is_empty() => format!("{identity}\n\n{prompt}"),
            _ => identity,
        };
        let system_prompt = build_system_prompt(project_instructions, Some(&agent_prompt));
        let mut full_messages = Vec::with_capacity(messages.len() + 1);
        if let Some(prompt) = system_prompt {
            full_messages.push(ChatMessage::text(ChatRole::System, prompt, None));
        }
        full_messages.extend(prepare_messages_for_agent(messages, &self.config.name));

        let sampling = self.config.sampling.clone().unwrap_or_default();
        let agent_name = self.config.name.clone();
        let client = Arc::clone(&self.client);
        let tools = Arc::clone(&self.tools);
        let max_rounds = max_tool_rounds.unwrap_or(DEFAULT_MAX_TOOL_ROUNDS);
        let approval_mode = self.approval_mode;
        let approval_cache = self.approval_cache.clone();
        let shell_allow_commands = self.shell_allow_commands.clone();

        // Spawn the HTTP request + stream consumption + tool loop so the
        // event loop is free to redraw immediately.
        tokio::spawn(async move {
            // Build retry callback that forwards retry info to the TUI.
            let tx_retry = tx.clone();
            let on_retry = move |info: krew_llm::common::RetryInfo| {
                let _ = tx_retry.send(AgentEvent::Retrying {
                    attempt: info.attempt,
                    max_attempts: info.max_attempts,
                    reason: info.reason.clone(),
                    delay_secs: info.delay_secs,
                });
            };

            // Convert ToolSpec -> ToolDefinition for the LLM API.
            let tool_defs: Vec<ToolDefinition> = tools
                .specs()
                .iter()
                .map(|spec| ToolDefinition {
                    name: spec.name.clone(),
                    description: spec.description.clone(),
                    parameters: spec.parameters.clone(),
                })
                .collect();

            let ctx = AgentLoopContext {
                client: &client,
                tools: &tools,
                tool_defs: &tool_defs,
                sampling: &sampling,
                on_retry: &on_retry,
                tx: &tx,
                agent_name: &agent_name,
                max_rounds,
                approval_mode,
                approval_cache: &approval_cache,
                shell_allow_commands: &shell_allow_commands,
            };
            run_agent_loop(&ctx, &mut full_messages).await;
        });

        rx
    }
}

/// Context for a single agent loop execution, grouping all shared references.
struct AgentLoopContext<'a> {
    client: &'a Arc<dyn LlmClient>,
    tools: &'a ToolRegistry,
    tool_defs: &'a [ToolDefinition],
    sampling: &'a krew_config::SamplingConfig,
    on_retry: &'a (dyn Fn(krew_llm::common::RetryInfo) + Send + Sync),
    tx: &'a mpsc::UnboundedSender<AgentEvent>,
    agent_name: &'a str,
    max_rounds: u32,
    approval_mode: ApprovalMode,
    approval_cache: &'a ApprovalCache,
    shell_allow_commands: &'a [String],
}

/// Run the agent's tool-call loop: stream LLM → execute tools → re-call LLM.
///
/// The loop exits when the LLM finishes without tool calls, when the
/// maximum number of tool rounds is reached, or on error.
async fn run_agent_loop(ctx: &AgentLoopContext<'_>, messages: &mut Vec<ChatMessage>) {
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

/// Preprocess messages for a specific agent: keep own tool chains in native
/// format, convert other agents' tool chains to text descriptions.
///
/// Other agents' assistant+tool_calls messages are merged with their
/// subsequent Tool result messages into a single text Assistant message.
/// This allows every agent to see what tools other agents used, without
/// requiring native tool_calls format (which only works for the "self"
/// role).
fn prepare_messages_for_agent(messages: Vec<ChatMessage>, self_name: &str) -> Vec<ChatMessage> {
    let mut result = Vec::new();
    // Accumulates text for an other-agent's tool call block being folded.
    let mut pending_summary: Option<(String, String)> = None; // (agent_name, text)

    for msg in messages {
        match msg.role {
            ChatRole::Assistant if msg.tool_calls.is_some() => {
                // Flush any pending summary first.
                if let Some((name, text)) = pending_summary.take() {
                    result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
                }

                let is_other = msg.name.as_ref().is_some_and(|n| n != self_name);
                if is_other {
                    // Convert to text description for other agent visibility.
                    let agent_name = msg.name.clone().unwrap_or_default();
                    let mut text = msg.content.clone();
                    for tc in msg.tool_calls.as_ref().unwrap() {
                        let display = format_tool_call_text(&tc.name, &tc.arguments);
                        if !text.is_empty() {
                            text.push('\n');
                        }
                        text.push_str(&format!("[Used tool: {display}]"));
                    }
                    pending_summary = Some((agent_name, text));
                } else {
                    result.push(msg); // Keep native format for self.
                }
            }
            ChatRole::Tool if pending_summary.is_some() => {
                // Fold tool result into pending summary text.
                let (_, text) = pending_summary.as_mut().unwrap();
                let tool_name = msg.name.as_deref().unwrap_or("tool");
                text.push_str(&format!("\n[Result from {tool_name}: {}]", msg.content));
            }
            _ => {
                // Flush pending summary before pushing other messages.
                if let Some((name, text)) = pending_summary.take() {
                    result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
                }
                result.push(msg);
            }
        }
    }

    // Flush remaining pending summary.
    if let Some((name, text)) = pending_summary.take() {
        result.push(ChatMessage::text(ChatRole::Assistant, text, Some(name)));
    }

    result
}

/// Format a tool call as a plain text string: `tool_name("arg1", key="arg2")`
fn format_tool_call_text(name: &str, arguments: &str) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    let params = match args.as_object() {
        Some(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .map(|(key, val)| {
                    let display = match val {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        other => other.to_string(),
                    };
                    if obj.keys().next() == Some(key) {
                        display
                    } else {
                        format!("{key}={display}")
                    }
                })
                .collect();
            parts.join(", ")
        }
        None => String::new(),
    };
    format!("{name}({params})")
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

/// Result of checking whether a tool call needs user approval.
enum ToolApproval {
    /// Tool can be executed without asking the user.
    Auto,
    /// Tool requires user approval before execution.
    NeedsApproval {
        /// Whether the "Approve for Session" option should be shown.
        allow_session_approval: bool,
    },
}

/// Check whether a tool call needs user approval, considering:
/// - The tool's intrinsic approval requirement
/// - The global approval mode
/// - For shell tools: the extracted command prefixes, allowlist, and cache
async fn check_tool_approval(
    tool_name: &str,
    arguments: &str,
    tools: &ToolRegistry,
    mode: ApprovalMode,
    cache: &ApprovalCache,
    shell_allow_commands: &[String],
) -> ToolApproval {
    // Readonly tools never need approval.
    if !tools.requires_approval(tool_name) {
        return ToolApproval::Auto;
    }

    // FullAuto mode skips all approval.
    if mode == ApprovalMode::FullAuto {
        return ToolApproval::Auto;
    }

    // AutoEdit mode auto-approves write tools (only shell needs approval).
    if mode == ApprovalMode::AutoEdit && tool_name != "shell" {
        return ToolApproval::Auto;
    }

    // For non-shell tools: simple tool-name-based approval.
    if tool_name != "shell" {
        if cache.is_approved(tool_name).await {
            return ToolApproval::Auto;
        }
        return ToolApproval::NeedsApproval {
            allow_session_approval: true,
        };
    }

    // Shell tool: command-level approval.
    let command = extract_shell_command(arguments);
    let Some(command) = command else {
        // Cannot parse arguments — require approval, no session option.
        return ToolApproval::NeedsApproval {
            allow_session_approval: false,
        };
    };

    let prefixes = krew_tools::builtin::extract_command_prefixes(&command);
    let Some(prefixes) = prefixes else {
        // Complex command — require approval, no session option.
        return ToolApproval::NeedsApproval {
            allow_session_approval: false,
        };
    };

    // Check all prefixes against allowlist and cache.
    let mut all_approved = true;
    for prefix in &prefixes {
        // Check allowlist.
        let in_allowlist = shell_allow_commands
            .iter()
            .any(|entry| krew_tools::builtin::matches_allowlist_entry(prefix, entry));
        if in_allowlist {
            continue;
        }
        // Check session cache (key: "shell:<prefix>").
        let cache_key = format!("shell:{prefix}");
        if !cache.is_approved(&cache_key).await {
            all_approved = false;
            break;
        }
    }

    if all_approved {
        ToolApproval::Auto
    } else {
        ToolApproval::NeedsApproval {
            allow_session_approval: true,
        }
    }
}

/// Cache session approval for a tool call.
///
/// For shell tools, caches each extracted command prefix separately
/// (e.g. `shell:cargo build`). For other tools, caches by tool name.
async fn cache_session_approval(tool_name: &str, arguments: &str, cache: &ApprovalCache) {
    if tool_name == "shell"
        && let Some(command) = extract_shell_command(arguments)
        && let Some(prefixes) = krew_tools::builtin::extract_command_prefixes(&command)
    {
        for prefix in prefixes {
            let key = format!("shell:{prefix}");
            cache.approve_for_session(key).await;
        }
        return;
    }
    // Non-shell tools or shell parse failure: cache by tool name.
    cache.approve_for_session(tool_name.to_string()).await;
}

/// Extract the shell command string from a tool call's JSON arguments.
fn extract_shell_command(arguments: &str) -> Option<String> {
    let args: serde_json::Value = serde_json::from_str(arguments).ok()?;
    args.get("command")?.as_str().map(|s| s.to_string())
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

/// Build the final system prompt by merging project instructions with the
/// agent's configured system_prompt.
///
/// When project instructions are present, they are wrapped in
/// `<project-instructions>` tags and prepended before the agent's own prompt.
pub fn build_system_prompt(
    project_instructions: Option<&str>,
    agent_system_prompt: Option<&str>,
) -> Option<String> {
    match (project_instructions, agent_system_prompt) {
        (Some(instructions), Some(prompt)) if !prompt.is_empty() => Some(format!(
            "<project-instructions>\n{instructions}\n</project-instructions>\n\n{prompt}"
        )),
        (Some(instructions), _) => Some(format!(
            "<project-instructions>\n{instructions}\n</project-instructions>"
        )),
        (None, Some(prompt)) if !prompt.is_empty() => Some(prompt.to_string()),
        _ => None,
    }
}

/// Result of agent initialization.
pub struct InitAgentsResult {
    /// Agent runtimes keyed by agent name.
    pub agents: HashMap<String, AgentRuntime>,
    /// Warning messages for agents that could not be initialized.
    pub warnings: Vec<String>,
}

/// Build `AgentRuntime` instances from configuration.
///
/// Iterates over `config.agents`, resolves API keys (from config value or
/// environment variable), creates provider-specific `LlmClient` instances,
/// and constructs `AgentRuntime` for each valid agent.
///
/// When `cwd` is provided and `agent_config.tools` is true, readonly built-in
/// tools are registered for the agent.
///
/// Agents that fail initialization (missing API key, unknown provider) are
/// skipped with a warning message returned in `InitAgentsResult::warnings`.
pub fn init_agents(config: &Config, cwd: Option<PathBuf>) -> InitAgentsResult {
    let mut agents = HashMap::new();
    let mut warnings = Vec::new();
    let shared_approval_cache = ApprovalCache::new();

    for agent_config in &config.agents {
        if agent_config.provider == "builtin" {
            // Skip builtin echo agents — they don't need an LLM client.
            continue;
        }

        let provider_config = match config.providers.get(&agent_config.provider) {
            Some(p) => p,
            None => {
                warnings.push(format!(
                    "Agent '{}': provider '{}' not found, skipped",
                    agent_config.name, agent_config.provider
                ));
                continue;
            }
        };

        // Resolve API key: api_key takes precedence over api_key_env.
        let api_key = if let Some(key) = &provider_config.api_key {
            if key.is_empty() {
                warnings.push(format!(
                    "Agent '{}': api_key is empty, skipped",
                    agent_config.name
                ));
                continue;
            }
            key.clone()
        } else if let Some(env) = &provider_config.api_key_env {
            match std::env::var(env) {
                Ok(val) if !val.is_empty() => val,
                _ => {
                    warnings.push(format!(
                        "Agent '{}': env var '{}' not set or empty, skipped",
                        agent_config.name, env
                    ));
                    continue;
                }
            }
        } else {
            warnings.push(format!(
                "Agent '{}': no api_key or api_key_env configured, skipped",
                agent_config.name
            ));
            continue;
        };

        // Build shared client config.
        let client_config = LlmClientConfig {
            agent_name: agent_config.name.clone(),
            model: agent_config.model.clone(),
            api_key,
            base_url: provider_config.base_url.clone(),
            other_agent_role: config.settings.other_agent_role,
            retry_config: config.settings.retry.clone(),
            enable_thinking: agent_config.enable_thinking,
            thinking_effort: agent_config.thinking_effort,
        };

        // Create LLM client based on provider type.
        let client: Arc<dyn LlmClient> = match provider_config.provider_type {
            ProviderType::OpenAI => {
                let api_type = agent_config.api_type.unwrap_or(ApiType::Chat);
                match api_type {
                    ApiType::Chat => Arc::new(OpenAiChatClient::new(client_config)),
                    ApiType::Responses => Arc::new(OpenAiResponsesClient::new(client_config)),
                }
            }
            ProviderType::Anthropic => Arc::new(AnthropicClient::new(client_config)),
            ProviderType::Google => Arc::new(GoogleClient::new(
                client_config,
                provider_config.vertex_project.as_deref(),
                provider_config.vertex_location.as_deref(),
            )),
        };

        // Create tool registry for this agent.
        let tools = if agent_config.tools {
            if let Some(ref cwd) = cwd {
                Arc::new(krew_tools::builtin::create_full_registry(cwd.clone()))
            } else {
                Arc::new(ToolRegistry::empty())
            }
        } else {
            Arc::new(ToolRegistry::empty())
        };

        let runtime = AgentRuntime {
            config: agent_config.clone(),
            client,
            tools,
            is_responding: false,
            other_agent_role: config.settings.other_agent_role,
            approval_mode: config.settings.approval_mode,
            approval_cache: shared_approval_cache.clone(),
            shell_allow_commands: config.settings.shell_allow_commands.clone(),
        };

        agents.insert(agent_config.name.clone(), runtime);
    }

    InitAgentsResult { agents, warnings }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a simple text-only assistant message.
    fn assistant_msg(name: &str, text: &str) -> ChatMessage {
        ChatMessage::text(ChatRole::Assistant, text, Some(name.to_string()))
    }

    /// Helper to create an assistant message with tool calls.
    fn assistant_with_tools(name: &str, text: &str, tools: Vec<ToolCallInfo>) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            content: text.to_string(),
            name: Some(name.to_string()),
            tool_calls: Some(tools),
            tool_call_id: None,
        }
    }

    /// Helper to create a tool result message.
    fn tool_result(tool_name: &str, content: &str, call_id: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Tool,
            content: content.to_string(),
            name: Some(tool_name.to_string()),
            tool_calls: None,
            tool_call_id: Some(call_id.to_string()),
        }
    }

    fn tc(id: &str, name: &str, args: &str) -> ToolCallInfo {
        ToolCallInfo {
            id: id.to_string(),
            name: name.to_string(),
            arguments: args.to_string(),
            thought_signature: None,
        }
    }

    #[test]
    fn own_tool_chain_preserved_native() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read the file", None),
            assistant_with_tools(
                "agent_a",
                "Let me check",
                vec![tc("1", "read_file", r#"{"path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "fn main() {}", "1"),
            assistant_msg("agent_a", "The file has 1 line"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_a");

        assert_eq!(result.len(), 4);
        // Assistant with tool_calls should be preserved as-is.
        assert!(result[1].tool_calls.is_some());
        assert_eq!(result[1].tool_calls.as_ref().unwrap()[0].name, "read_file");
        // Tool result should be preserved as-is.
        assert_eq!(result[2].role, ChatRole::Tool);
        assert_eq!(result[2].content, "fn main() {}");
        // Final text message should be preserved.
        assert_eq!(result[3].content, "The file has 1 line");
    }

    #[test]
    fn other_agent_tool_chain_converted_to_text() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read the file", None),
            assistant_with_tools(
                "agent_a",
                "Let me check",
                vec![tc("1", "read_file", r#"{"path":"src/main.rs"}"#)],
            ),
            tool_result("read_file", "fn main() {}", "1"),
            assistant_msg("agent_a", "The file has 1 line"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        assert_eq!(result.len(), 3); // user, folded assistant, final text
        // Folded message should be text-only (no tool_calls).
        assert!(result[1].tool_calls.is_none());
        assert_eq!(result[1].role, ChatRole::Assistant);
        assert!(result[1].content.contains("[Used tool:"));
        assert!(result[1].content.contains("read_file"));
        assert!(result[1].content.contains("[Result from read_file:"));
        assert!(result[1].content.contains("fn main() {}"));
        assert_eq!(result[1].name.as_deref(), Some("agent_a"));
        // Final text preserved.
        assert_eq!(result[2].content, "The file has 1 line");
    }

    #[test]
    fn messages_without_tools_unaffected() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "hello", None),
            assistant_msg("agent_a", "hi there"),
            ChatMessage::text(ChatRole::User, "how are you?", None),
            assistant_msg("agent_b", "I am fine"),
        ];

        let result = prepare_messages_for_agent(messages.clone(), "agent_a");

        assert_eq!(result.len(), 4);
        for (orig, processed) in messages.iter().zip(result.iter()) {
            assert_eq!(orig.content, processed.content);
            assert_eq!(orig.role, processed.role);
        }
    }

    #[test]
    fn multiple_tool_calls_folded_correctly() {
        let messages = vec![
            assistant_with_tools(
                "agent_a",
                "",
                vec![
                    tc("1", "glob", r#"{"pattern":"*.rs"}"#),
                    tc("2", "grep", r#"{"pattern":"main"}"#),
                ],
            ),
            tool_result("glob", "found 3 files", "1"),
            tool_result("grep", "2 matches", "2"),
            assistant_msg("agent_a", "Done scanning"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        assert_eq!(result.len(), 2); // folded + final text
        let folded = &result[0];
        assert!(folded.content.contains("[Used tool: glob"));
        assert!(folded.content.contains("[Used tool: grep"));
        assert!(folded.content.contains("[Result from glob: found 3 files]"));
        assert!(folded.content.contains("[Result from grep: 2 matches]"));
    }

    #[test]
    fn mixed_agents_own_and_other() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read files", None),
            // Agent A uses a tool (other agent for agent_b).
            assistant_with_tools(
                "agent_a",
                "Checking",
                vec![tc("1", "read_file", r#"{"path":"a.rs"}"#)],
            ),
            tool_result("read_file", "content_a", "1"),
            assistant_msg("agent_a", "Found it"),
            // Agent B uses a tool (self for agent_b).
            assistant_with_tools(
                "agent_b",
                "Let me also check",
                vec![tc("2", "read_file", r#"{"path":"b.rs"}"#)],
            ),
            tool_result("read_file", "content_b", "2"),
            assistant_msg("agent_b", "Got it"),
        ];

        let result = prepare_messages_for_agent(messages, "agent_b");

        // user + folded(agent_a) + text(agent_a) + native_tc(agent_b) + tool(agent_b) + text(agent_b)
        assert_eq!(result.len(), 6);
        // Agent A's tool call should be folded to text.
        assert!(result[1].tool_calls.is_none());
        assert!(result[1].content.contains("[Used tool:"));
        // Agent B's tool call should be native.
        assert!(result[3].tool_calls.is_some());
        assert_eq!(result[4].role, ChatRole::Tool);
    }

    // -- Approval policy tests ------------------------------------------------

    fn test_registry() -> ToolRegistry {
        use std::path::PathBuf;
        krew_tools::builtin::create_full_registry(PathBuf::from("/tmp"))
    }

    /// Helper to check approval result.
    async fn is_auto(
        tool_name: &str,
        arguments: &str,
        registry: &ToolRegistry,
        mode: ApprovalMode,
        cache: &ApprovalCache,
        allow_cmds: &[String],
    ) -> bool {
        matches!(
            check_tool_approval(tool_name, arguments, registry, mode, cache, allow_cmds).await,
            ToolApproval::Auto
        )
    }

    #[tokio::test]
    async fn suggest_mode_readonly_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "read_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "glob",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "grep",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn suggest_mode_write_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            !is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            !is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Shell with non-allowlisted command.
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_write_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn auto_edit_mode_shell_needs_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            !is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::AutoEdit,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn full_auto_mode_all_auto() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "read_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "write_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        assert!(
            is_auto(
                "edit_file",
                "{}",
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
        let shell_args = r#"{"command":"rm -rf /"}"#;
        assert!(
            is_auto(
                "shell",
                shell_args,
                &registry,
                ApprovalMode::FullAuto,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn unknown_tool_no_approval() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        assert!(
            is_auto(
                "unknown_tool",
                "{}",
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_allowlist_auto_approves() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec!["ls".to_string(), "cargo build".to_string()];
        // ls is in allowlist.
        let args = r#"{"command":"ls -la"}"#;
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // cargo build is in allowlist.
        let args = r#"{"command":"cargo build --release"}"#;
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // cargo test is NOT in allowlist (only cargo build is).
        let args = r#"{"command":"cargo test"}"#;
        assert!(
            !is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_session_cache_by_prefix() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        // Initially needs approval.
        let args = r#"{"command":"cargo build --release"}"#;
        assert!(
            !is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Cache "cargo build" for session.
        cache_session_approval("shell", args, &cache).await;
        // Now auto-approved.
        assert!(
            is_auto(
                "shell",
                args,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Same prefix with different flags also auto-approved.
        let args2 = r#"{"command":"cargo build -p krew-core"}"#;
        assert!(
            is_auto(
                "shell",
                args2,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
        // Different subcommand still needs approval.
        let args3 = r#"{"command":"cargo test"}"#;
        assert!(
            !is_auto(
                "shell",
                args3,
                &registry,
                ApprovalMode::Suggest,
                &cache,
                &allow
            )
            .await
        );
    }

    #[tokio::test]
    async fn shell_complex_command_no_session_option() {
        let registry = test_registry();
        let cache = ApprovalCache::new();
        let allow = vec![];
        // Complex command with command substitution.
        let args = r#"{"command":"echo $(whoami)"}"#;
        let result = check_tool_approval(
            "shell",
            args,
            &registry,
            ApprovalMode::Suggest,
            &cache,
            &allow,
        )
        .await;
        match result {
            ToolApproval::NeedsApproval {
                allow_session_approval,
            } => {
                assert!(!allow_session_approval);
            }
            ToolApproval::Auto => panic!("expected NeedsApproval"),
        }
    }
}
