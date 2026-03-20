//! Non-interactive prompt mode (-p flag).
//!
//! Runs a single prompt against one or more agents, outputs results to stdout,
//! and exits. Shares all krew-core logic with the TUI mode.

use std::collections::{HashMap, HashSet};
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use krew_config::{ApprovalMode, Config};
use krew_core::agent::{AgentRuntime, PeerAgent, init_agents};
use krew_core::event::{AgentEvent, ReviewDecision};
use krew_core::persistence::{SessionSnapshot, build_session_file};
use krew_core::router::{self, Addressee};
use krew_llm::{ChatMessage, ChatRole, Usage};
use krew_tools::mcp::McpManager;

/// Output format for prompt mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Text,
    Json,
}

/// Run prompt mode: send a single prompt to agents and output results.
///
/// Returns an exit code: 0 = success, 1 = agent error, 2 = argument/config error.
pub async fn run_prompt_mode(
    mut config: Config,
    cwd: PathBuf,
    raw_prompt: String,
    format: OutputFormat,
) -> i32 {
    // Normalize config (auto-append missing agents to reply_order).
    config.normalize();

    // Parse addressee from the raw prompt only (not stdin).
    let agent_names: Vec<String> = config.agents.iter().map(|a| a.name.clone()).collect();
    let (addressee, _body, is_whisper) = match router::parse_input(&raw_prompt, &agent_names) {
        Ok(result) => result,
        Err(e) => {
            eprintln!("Error: {e}");
            return 2;
        }
    };

    // Reject LastRespondent — prompt mode requires explicit @agent or @all.
    if matches!(addressee, Addressee::LastRespondent) {
        eprintln!("Error: Prompt mode requires @agent or @all addressing");
        return 2;
    }

    // Read stdin if piped, build final message body.
    let message_body = build_message_body(&raw_prompt);

    // Initialize agents.
    let init_result = init_agents(&config, Some(cwd.clone()));
    let mut agents = init_result.agents;

    if agents.is_empty() {
        eprintln!("Error: No agents could be initialized");
        for w in &init_result.warnings {
            eprintln!("  {w}");
        }
        return 2;
    }

    // Force FullAuto approval mode on all agents.
    for agent in agents.values_mut() {
        agent.approval_mode = ApprovalMode::FullAuto;
    }

    // Initialize MCP servers if configured.
    let _mcp_manager = if !config.mcp_servers.is_empty() {
        let manager = McpManager::start_all(&config.mcp_servers).await;
        for err in manager.errors() {
            eprintln!("Warning: {err}");
        }
        if manager.server_count() > 0 {
            for runtime in agents.values_mut() {
                if runtime.config.tools
                    && let Some(registry) = Arc::get_mut(&mut runtime.tools)
                {
                    manager.register_tools(registry);
                }
            }
        }
        Some(manager)
    } else {
        None
    };

    // Load project instructions.
    let project_instructions = krew_config::load_project_instructions(&cwd).ok().flatten();

    // Build dispatch queue.
    let available: HashSet<String> = agents.keys().cloned().collect();
    let mut pending =
        router::resolve_dispatch_queue(&addressee, &config.settings.reply_order, &available, None);

    // Check for unavailable agents.
    let unavailable: Vec<String> = pending
        .iter()
        .filter(|name| !agents.contains_key(name.as_str()))
        .cloned()
        .collect();
    if !unavailable.is_empty() {
        pending.retain(|name| agents.contains_key(name.as_str()));
        eprintln!("Warning: Agent unavailable: {}", unavailable.join(", "));
        if pending.is_empty() {
            eprintln!("Error: No available agents to process the request");
            return 2;
        }
    }

    // Track whisper state for dispatch lifecycle.
    let current_whisper_targets: Option<Vec<String>> = if is_whisper {
        let available: HashSet<String> = agents.keys().cloned().collect();
        let target_names = router::resolve_target_names(
            &addressee,
            &config.settings.reply_order,
            &available,
            None,
        );
        Some(target_names.into_iter().map(String::from).collect())
    } else {
        None
    };

    // Build user message and conversation state.
    let addressee_str = match &addressee {
        Addressee::All => Some("all".to_string()),
        Addressee::Single(name) => Some(name.clone()),
        Addressee::Multiple(names) => Some(names.join(",")),
        Addressee::LastRespondent => None,
    };
    let mut messages = vec![
        ChatMessage::user_with_addressee(message_body, addressee_str)
            .with_whisper_targets(current_whisper_targets.clone()),
    ];
    let mut token_usage: HashMap<String, (u32, u32)> = HashMap::new();
    let mut has_error = false;
    let mut ai_conversation_rounds: u32 = 0;
    let mut a2a_insert_cursor: usize = 0;
    let mut is_first_agent = true;

    // Persist stable session state for crash recovery.
    let session_dir = cwd.join(".krew").join("sessions");
    if let Err(e) = std::fs::create_dir_all(&session_dir) {
        tracing::warn!(error = %e, "Failed to create sessions directory");
    }
    let session_ctx = SessionContext {
        session_dir,
        session_id: uuid::Uuid::new_v4().to_string()[..8].to_string(),
        created_at: chrono::Utc::now(),
        cwd: &cwd,
        config: &config,
        agents: &agents,
    };

    // Save session after user message (matching TUI behavior).
    session_ctx.save(&messages, &token_usage);

    // Process agents sequentially.
    while let Some(agent_name) = pending.pop_front() {
        a2a_insert_cursor = a2a_insert_cursor.saturating_sub(1);

        let agent = match agents.get(&agent_name) {
            Some(a) => a,
            None => continue,
        };

        // Build peer agent list for AI-to-AI routing.
        let peers = if config.settings.agent_to_agent_max_rounds > 0 {
            Some(
                agents
                    .values()
                    .filter(|a| a.config.name != agent_name)
                    .map(|a| PeerAgent {
                        name: a.config.name.clone(),
                        display_name: a.config.display_name.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
        } else {
            None
        };

        let mut rx = agent.start_completion(
            messages.clone(),
            project_instructions.as_deref(),
            None,
            peers.as_deref(),
            current_whisper_targets.clone(),
        );

        // Blank line between agents (not before the first).
        if !is_first_agent && format == OutputFormat::Text {
            println!();
        }
        is_first_agent = false;

        // Consume events from this agent.
        let result = consume_agent_events(
            &mut rx,
            &agent_name,
            format,
            current_whisper_targets.is_some(),
            current_whisper_targets.clone(),
        )
        .await;

        // Update state from result.
        if result.has_error {
            has_error = true;
        }

        if let Some(usage) = &result.usage {
            let entry = token_usage.entry(agent_name.clone()).or_insert((0, 0));
            entry.0 += usage.prompt_tokens;
            entry.1 += usage.completion_tokens;
        }

        // Persist messages.
        messages.extend(result.intermediate_messages);

        // Persist the final assistant message:
        // - On success: always persist (even if text is empty).
        // - On error with partial text: persist with [Error: ...] appended
        //   (already done by consume_agent_events, matching TUI behavior).
        // - On error with no text: skip (nothing useful to persist).
        if !result.has_error || !result.final_text.is_empty() {
            let mut final_msg = ChatMessage::text(
                ChatRole::Assistant,
                result.final_text.clone(),
                Some(agent_name.clone()),
            );
            final_msg.server_tool_uses = result.server_tool_uses;
            final_msg.whisper_targets = current_whisper_targets.clone();
            if let Some(usage) = result.usage {
                final_msg.usage = Some(usage);
            }
            messages.push(final_msg);
        }

        // Save session after each agent completes/errors (crash recovery).
        session_ctx.save(&messages, &token_usage);

        // AI-to-AI routing: detect @mentions in the response.
        if config.settings.agent_to_agent_max_rounds > 0 && !result.has_error {
            let known: Vec<String> = agents.keys().cloned().collect();
            let targets = router::parse_agent_mentions(&result.final_text, &known, &agent_name);
            // In whisper mode, filter A2A targets to only include whisper group members.
            let targets = if let Some(ref wt) = current_whisper_targets {
                targets
                    .into_iter()
                    .filter(|t| wt.contains(t))
                    .collect::<Vec<_>>()
            } else {
                targets
            };
            for target in targets {
                if ai_conversation_rounds >= config.settings.agent_to_agent_max_rounds {
                    break;
                }
                ai_conversation_rounds += 1;
                match config.settings.agent_to_agent_routing {
                    krew_config::AgentToAgentRouting::Immediate => {
                        router::apply_immediate_routing_at(
                            &mut pending,
                            &target,
                            &mut a2a_insert_cursor,
                        );
                    }
                    krew_config::AgentToAgentRouting::Queued => {
                        router::apply_queued_routing(&mut pending, &target);
                    }
                }
            }
        }
    }

    if has_error { 1 } else { 0 }
}

/// Build the final message body by optionally prepending stdin content.
fn build_message_body(raw_prompt: &str) -> String {
    if std::io::stdin().is_terminal() {
        return raw_prompt.to_string();
    }

    // Read all stdin.
    let mut stdin_content = String::new();
    if let Err(e) = std::io::stdin().read_line(&mut stdin_content) {
        eprintln!("Warning: failed to read stdin: {e}");
        return raw_prompt.to_string();
    }
    // Read remaining lines.
    let mut rest = String::new();
    while std::io::stdin().read_line(&mut rest).unwrap_or(0) > 0 {
        stdin_content.push_str(&rest);
        rest.clear();
    }

    combine_stdin_and_prompt(&stdin_content, raw_prompt)
}

/// Combine stdin content with the raw prompt.
///
/// If stdin is empty (after trimming), returns just the prompt.
/// Otherwise wraps stdin in `<stdin>...</stdin>` tags and prepends it.
pub(crate) fn combine_stdin_and_prompt(stdin_content: &str, raw_prompt: &str) -> String {
    let trimmed = stdin_content.trim_end();
    if trimmed.is_empty() {
        return raw_prompt.to_string();
    }
    format!("<stdin>\n{trimmed}\n</stdin>\n\n{raw_prompt}")
}

/// Result of consuming all events from a single agent.
struct AgentResult {
    final_text: String,
    intermediate_messages: Vec<ChatMessage>,
    server_tool_uses: Vec<krew_llm::ServerToolUseInfo>,
    usage: Option<Usage>,
    has_error: bool,
}

/// Consume all events from a single agent's event channel.
async fn consume_agent_events(
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AgentEvent>,
    agent_name: &str,
    format: OutputFormat,
    is_whisper: bool,
    whisper_targets_for_json: Option<Vec<String>>,
) -> AgentResult {
    let mut text_buffer = String::new();
    let mut intermediate_messages = Vec::new();
    let mut server_tool_uses = Vec::new();
    let mut usage = None;
    let mut has_error = false;
    let mut error_message = None;
    let mut stdout = std::io::stdout();
    // Track whether we are mid-line in text streaming mode (after print!
    // without a trailing newline). Non-text events must flush a newline first.
    let mut mid_line = false;
    // Track server tool state for Gemini-style interleaved text (matching TUI).
    let mut server_tool_started = false;
    let mut text_after_server_tool = false;

    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::ResponseStart {
                agent_name: name, ..
            } => {
                if format == OutputFormat::Text {
                    if is_whisper {
                        println!("[{name}] [whisper]");
                    } else {
                        println!("[{name}]");
                    }
                }
            }
            AgentEvent::ThinkingDelta(_) => {
                // Silently discard thinking content. Unlike TUI (which renders
                // thinking), we do NOT set text_after_server_tool here — the
                // user sees no content, so server tool display should stay in
                // the "start/done adjacent" style.
            }
            AgentEvent::TextDelta(text) => {
                if server_tool_started {
                    text_after_server_tool = true;
                }
                text_buffer.push_str(&text);
                if format == OutputFormat::Text {
                    print!("{text}");
                    let _ = stdout.flush();
                    mid_line = !text.ends_with('\n');
                }
            }
            AgentEvent::ServerToolStart { name } => {
                if format == OutputFormat::Text && mid_line {
                    println!();
                    mid_line = false;
                }
                match format {
                    OutputFormat::Text => {
                        // Skip display for google_search (only show done),
                        // matching TUI behavior.
                        if name != "google_search" {
                            println!("\u{1F310} {name}");
                        }
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "agent": agent_name,
                                "type": "server_tool_start",
                                "tool": name,
                            })
                        );
                    }
                }
                server_tool_started = true;
                text_after_server_tool = false;
            }
            AgentEvent::ServerToolDone { name, query } => {
                let had_text_between = text_after_server_tool;
                server_tool_started = false;
                text_after_server_tool = false;

                // Display only — don't collect here. Done event carries the
                // authoritative server_tool_uses list across all rounds.
                match format {
                    OutputFormat::Text => {
                        if had_text_between {
                            // Gemini pattern: text was emitted between start and done.
                            // Flush mid-line text, then show 🌐 name_done("query").
                            if mid_line {
                                println!();
                                mid_line = false;
                            }
                            let done_name = format!("{name}_done");
                            let suffix = query
                                .as_ref()
                                .map(|q| format!("(\"{q}\")"))
                                .unwrap_or_default();
                            println!("\u{1F310} {done_name}{suffix}");
                            println!();
                        } else {
                            // OpenAI/Anthropic pattern: start and done are adjacent.
                            let summary = query
                                .as_ref()
                                .map(|q| format!("\"{q}\""))
                                .unwrap_or_default();
                            println!("   \u{23BF}  {summary}");
                            println!();
                        }
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "agent": agent_name,
                                "type": "server_tool_done",
                                "tool": name,
                                "query": query,
                            })
                        );
                    }
                }
            }
            AgentEvent::ToolCallStart { name, arguments } => {
                if format == OutputFormat::Text && mid_line {
                    println!();
                    mid_line = false;
                }
                match format {
                    OutputFormat::Text => {
                        let args_preview = format_tool_args_preview(&name, &arguments);
                        println!("\u{26A1} {name}({args_preview})");
                    }
                    OutputFormat::Json => {
                        println!(
                            "{}",
                            serde_json::json!({
                                "agent": agent_name,
                                "type": "tool_start",
                                "tool": name,
                                "arguments": arguments,
                            })
                        );
                    }
                }
            }
            AgentEvent::ToolCallOutput { text } => match format {
                OutputFormat::Text => {
                    println!("    {text}");
                }
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "agent": agent_name,
                            "type": "tool_output",
                            "text": text,
                        })
                    );
                }
            },
            AgentEvent::ToolCallDone {
                name,
                result_summary,
            } => match format {
                OutputFormat::Text => {
                    println!("   \u{23BF}  {result_summary}");
                    println!();
                }
                OutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::json!({
                            "agent": agent_name,
                            "type": "tool_done",
                            "tool": name,
                            "summary": result_summary,
                        })
                    );
                }
            },
            AgentEvent::Done {
                usage: u,
                intermediate_messages: msgs,
                final_text,
                server_tool_uses: stus,
            } => {
                usage = Some(u);
                intermediate_messages = msgs;
                // Use only the authoritative list from Done (not local collection).
                server_tool_uses = stus;
                text_buffer = final_text;

                match format {
                    OutputFormat::Text => {
                        // Ensure trailing newline after streamed text.
                        if mid_line {
                            println!();
                            mid_line = false;
                        }
                    }
                    OutputFormat::Json => {
                        let mut json = serde_json::json!({
                            "agent": agent_name,
                            "type": "text",
                            "content": text_buffer,
                        });
                        if let Some(ref wt) = whisper_targets_for_json {
                            json["whisper_targets"] = serde_json::json!(wt);
                        }
                        println!("{json}");
                    }
                }
            }
            AgentEvent::Error {
                message,
                intermediate_messages: msgs,
            } => {
                if format == OutputFormat::Text && mid_line {
                    println!();
                    mid_line = false;
                }
                eprintln!("Error [{agent_name}]: {message}");
                has_error = true;
                error_message = Some(message);
                intermediate_messages = msgs;
            }
            AgentEvent::ApprovalRequest { respond, .. } => {
                // Auto-approve everything in prompt mode.
                let _ = respond.send(ReviewDecision::Approved);
            }
            AgentEvent::Retrying {
                attempt,
                max_attempts,
                reason,
                delay_secs,
            } => {
                if format == OutputFormat::Text && mid_line {
                    println!();
                    mid_line = false;
                }
                eprintln!(
                    "Retrying [{agent_name}] ({attempt}/{max_attempts}, {reason}, {delay_secs:.0}s)..."
                );
            }
        }
    }

    // If error occurred and there was partial text, append error annotation
    // (matching TUI behavior in app/state.rs:905).
    if has_error
        && let Some(err_msg) = &error_message
        && !text_buffer.is_empty()
    {
        text_buffer.push_str(&format!("\n\n[Error: {err_msg}]"));
    }

    AgentResult {
        final_text: text_buffer,
        intermediate_messages,
        server_tool_uses,
        usage,
        has_error,
    }
}

/// Format tool arguments as a short preview string for text output.
fn format_tool_args_preview(tool_name: &str, arguments: &str) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    match tool_name {
        "read_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "write_file" | "edit_file" => args
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "shell" => args
            .get("command")
            .and_then(|v| v.as_str())
            .map(|cmd| {
                if cmd.len() > 60 {
                    format!("{}...", &cmd[..57])
                } else {
                    cmd.to_string()
                }
            })
            .unwrap_or_default(),
        "glob" => args
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "grep" => args
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => {
            let s = arguments.to_string();
            if s.len() > 60 {
                format!("{}...", &s[..57])
            } else {
                s
            }
        }
    }
}

/// Stable session context shared across incremental saves.
struct SessionContext<'a> {
    session_dir: PathBuf,
    session_id: String,
    created_at: chrono::DateTime<chrono::Utc>,
    cwd: &'a Path,
    config: &'a Config,
    agents: &'a HashMap<String, AgentRuntime>,
}

impl SessionContext<'_> {
    /// Save the session to disk (overwrites same session file each time).
    fn save(&self, messages: &[ChatMessage], token_usage: &HashMap<String, (u32, u32)>) {
        let session_path = self.session_dir.join(format!("{}.toml", self.session_id));

        let agent_names: Vec<String> = self
            .config
            .agents
            .iter()
            .filter(|a| self.agents.contains_key(&a.name))
            .map(|a| a.name.clone())
            .collect();

        let snapshot = SessionSnapshot {
            session_id: &self.session_id,
            cwd: self.cwd,
            agent_names,
            messages,
            token_usage,
            created_at: self.created_at,
        };

        let session_file = build_session_file(&snapshot);

        if let Err(e) = krew_storage::session_file::save_session(&session_path, &session_file) {
            tracing::warn!(error = %e, "Failed to save session");
        }
    }
}

#[cfg(test)]
mod tests;
