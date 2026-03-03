//! App state machine and main event loop.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::{Notify, mpsc};

use krew_config::Config;
use krew_core::agent::{AgentRuntime, init_agents};
use krew_core::event::AgentEvent;
use krew_llm::{ChatMessage, ChatRole};

use crate::completion::ActivePopup;
use crate::custom_terminal;
use crate::frame_scheduler::FrameRequester;
use crate::render;
use crate::streaming::StreamState;
use crate::streaming::chunking::AdaptiveChunkingPolicy;
use crate::streaming::commit_tick::run_commit_tick;
use crate::streaming::markdown_stream::MarkdownStreamCollector;
use crate::textarea::TextArea;

use super::agent_display::format_tool_call_display;
use super::paste_burst::{FlushResult, PasteBurst};

/// Duration within which a second Ctrl+C triggers quit.
pub(super) const QUIT_SHORTCUT_TIMEOUT: Duration = Duration::from_secs(1);

/// Commit tick interval (~60 Hz).
const COMMIT_TICK_INTERVAL: Duration = Duration::from_millis(16);

/// Top-level application state.
pub struct App {
    /// Current working directory for the session.
    pub cwd: PathBuf,
    /// Loaded configuration.
    pub config: Config,
    /// Current session ID (first 8 chars of UUID).
    pub(crate) session_id: String,
    /// Path to `.krew/sessions/` directory.
    pub(crate) session_dir: PathBuf,
    /// Path to `.krew/history` file.
    pub(crate) history_path: PathBuf,
    /// Project-level instructions loaded from AGENTS.md files (if any).
    pub project_instructions: Option<String>,
    /// Multi-line text input component.
    pub textarea: TextArea,
    /// Whether the app should quit.
    pub should_quit: bool,
    /// Timestamp when the first Ctrl+C was pressed (for double-press detection).
    pub(crate) quit_shortcut_armed_at: Option<Instant>,
    /// Transient hint shown in the status bar.
    pub quit_hint: Option<String>,
    /// Active completion popup state.
    pub popup: ActivePopup,
    /// Input history (most recent last).
    pub(crate) history: Vec<String>,
    /// Current position in history navigation (None = not browsing).
    pub(crate) history_index: Option<usize>,
    /// Draft input saved when entering history navigation.
    pub(crate) history_draft: String,
    /// Non-bracketed paste burst tracker for Windows fallback.
    pub(crate) paste_burst: PasteBurst,
    /// Stored full text for large paste placeholders (element_id → actual text).
    pub(crate) pending_pastes: HashMap<u64, String>,
    /// Counter for paste placeholder display numbering.
    pub(crate) paste_counter: usize,
    /// Frame scheduler handle for coalesced rendering.
    pub(crate) frame_requester: Option<FrameRequester>,

    // --- Phase 4: Agent integration ---
    /// Agent runtimes keyed by agent name.
    pub(crate) agents: HashMap<String, AgentRuntime>,
    /// Conversation message history.
    pub(crate) messages: Vec<ChatMessage>,
    /// Active agent event receiver (Some while streaming).
    pub(crate) agent_event_rx: Option<mpsc::UnboundedReceiver<AgentEvent>>,
    /// Streaming pipeline: markdown collector.
    pub(crate) stream_collector: Option<MarkdownStreamCollector>,
    /// Streaming pipeline: line queue.
    pub(crate) stream_state: StreamState,
    /// Streaming pipeline: adaptive chunking policy.
    pub(crate) chunking_policy: AdaptiveChunkingPolicy,
    /// Whether commit tick animation is active.
    pub(crate) commit_tick_active: bool,
    /// Whether the agent is currently in thinking phase.
    pub(crate) is_thinking: bool,
    /// Accumulated response text for the current streaming agent.
    pub(crate) current_response_text: String,
    /// Name of the agent currently streaming.
    pub(crate) current_agent_name: Option<String>,
    /// Accumulated token usage per agent (agent_name → total_tokens).
    pub(crate) agent_token_usage: HashMap<String, (u32, u32)>,
    /// Startup warnings to display after header.
    pub(crate) startup_warnings: Vec<String>,
    /// Queue of agent names waiting to run (for @all / @multiple dispatch).
    pub(crate) pending_agents: VecDeque<String>,
    /// Name of the last agent that successfully responded (for LastRespondent routing).
    pub(crate) last_respondent: Option<String>,

    // --- Agent status indicator ---
    /// Timestamp when the current agent started processing (drives status line visibility).
    pub agent_start_time: Option<Instant>,
    /// Display name of the currently active agent (shown in status line).
    pub agent_display_name: Option<String>,
    /// Color name of the currently active agent (shown in status line).
    pub agent_color: Option<String>,
    /// Override text for the agent status line (e.g. retry status).
    pub agent_status_text: Option<String>,
    /// Session ID to resume on startup (set by --resume, consumed by run()).
    pub(crate) pending_resume_id: Option<String>,
}

impl App {
    /// Initialize the application with the given config and working directory.
    pub fn new(cwd: PathBuf, config: Config) -> anyhow::Result<Self> {
        let project_instructions = match krew_config::load_project_instructions(&cwd) {
            Ok(instructions) => instructions,
            Err(e) => {
                tracing::warn!(error = %e, "Failed to load project instructions");
                None
            }
        };

        // Initialize agent runtimes via krew-core.
        let init_result = init_agents(&config, Some(cwd.clone()));
        let agents = init_result.agents;
        for w in &init_result.warnings {
            tracing::warn!("{}", w);
        }

        // Session setup.
        let session_id = uuid::Uuid::new_v4().to_string()[..8].to_string();
        let krew_dir = cwd.join(".krew");
        let session_dir = krew_dir.join("sessions");
        let history_path = krew_dir.join("history");

        // Ensure sessions directory exists.
        if let Err(e) = std::fs::create_dir_all(&session_dir) {
            tracing::warn!(error = %e, "Failed to create sessions directory");
        }

        // Load input history from file.
        let history = krew_core::persistence::load_and_truncate_history(
            &history_path,
            config.settings.input_history_limit,
        );

        Ok(Self {
            cwd,
            config,
            session_id,
            session_dir,
            history_path,
            project_instructions,
            textarea: TextArea::new(),
            should_quit: false,
            quit_shortcut_armed_at: None,
            quit_hint: None,
            popup: ActivePopup::None,
            history,
            history_index: None,
            history_draft: String::new(),
            paste_burst: PasteBurst::default(),
            pending_pastes: HashMap::new(),
            paste_counter: 0,
            frame_requester: None,
            agents,
            messages: Vec::new(),
            agent_event_rx: None,
            stream_collector: None,
            stream_state: StreamState::new(),
            chunking_policy: AdaptiveChunkingPolicy::new(),
            commit_tick_active: false,
            is_thinking: false,
            current_response_text: String::new(),
            current_agent_name: None,
            agent_token_usage: HashMap::new(),
            startup_warnings: init_result.warnings,
            pending_agents: VecDeque::new(),
            last_respondent: None,
            agent_start_time: None,
            agent_display_name: None,
            agent_color: None,
            agent_status_text: None,
            pending_resume_id: None,
        })
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Print the header above the viewport (scrolls into scrollback).
        render::insert_header(terminal, self)?;

        // Display startup warnings.
        for warning in std::mem::take(&mut self.startup_warnings) {
            self.show_warning(terminal, &warning)?;
        }

        // Resume session if requested via --resume (replay history on screen).
        if let Some(session_id) = self.pending_resume_id.take()
            && let Err(e) = self.load_session(&session_id, terminal)
        {
            self.show_warning(terminal, &format!("Failed to resume session: {e}"))?;
        }

        // Set up the frame scheduler for coalesced rendering (max 120 FPS).
        let draw_signal = Arc::new(Notify::new());
        let frame_requester = FrameRequester::spawn(Arc::clone(&draw_signal));
        self.frame_requester = Some(frame_requester);

        // Schedule the initial frame.
        self.request_redraw();

        let mut event_stream = EventStream::new();

        // Commit tick interval for streaming animation.
        let mut commit_tick = tokio::time::interval(COMMIT_TICK_INTERVAL);
        commit_tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                // Branch 1: Terminal events (key, paste, resize).
                maybe_event = event_stream.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            self.handle_event(event, terminal)?;
                            self.request_redraw();
                        }
                        Some(Err(e)) => {
                            tracing::error!(error = %e, "Terminal event stream error");
                            break;
                        }
                        None => break,
                    }
                }

                // Branch 2: Agent events (streaming response).
                Some(agent_event) = async {
                    match &mut self.agent_event_rx {
                        Some(rx) => rx.recv().await,
                        None => std::future::pending().await,
                    }
                } => {
                    self.handle_agent_event(agent_event, terminal)?;
                    self.request_redraw();
                }

                // Branch 3: Commit tick (drives streaming queue drain).
                _ = commit_tick.tick(), if self.commit_tick_active => {
                    self.handle_commit_tick(terminal)?;
                    self.request_redraw();
                }

                // Branch 4: Draw frame (coalesced by scheduler, max 120 FPS).
                _ = draw_signal.notified() => {
                    // Skip render during active paste burst.
                    if self.handle_paste_burst_tick() {
                        continue;
                    }

                    // Check if quit hint has expired.
                    if let Some(armed_at) = self.quit_shortcut_armed_at
                        && armed_at.elapsed() >= QUIT_SHORTCUT_TIMEOUT
                    {
                        self.quit_shortcut_armed_at = None;
                        self.quit_hint = None;
                    }

                    // Sync completion popup based on current input.
                    self.sync_popup();

                    // Adjust viewport height to fit textarea + status line + popup.
                    let term_width = terminal.size()?.width.saturating_sub(2);
                    let input_lines = self.textarea.desired_height(term_width.max(1));
                    let status_line_height: u16 =
                        if self.agent_start_time.is_some() { 1 } else { 0 };
                    let needed =
                        input_lines.max(1) + 3 + status_line_height + self.popup.extra_height();
                    terminal.ensure_viewport_height(needed)?;

                    // Render input prompt + status bar inside the inline viewport.
                    terminal.draw(|frame| render::render_input_viewport(frame, self))?;
                }
            }

            if self.should_quit {
                break;
            }
        }

        Ok(())
    }

    /// Handle an incoming agent event.
    fn handle_agent_event(
        &mut self,
        event: AgentEvent,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        match event {
            AgentEvent::ResponseStart {
                agent_name,
                display_name,
                color,
            } => {
                self.current_agent_name = Some(agent_name.clone());
                self.current_response_text.clear();

                // Activate the agent status indicator line.
                self.agent_start_time = Some(Instant::now());
                self.agent_display_name = Some(display_name.clone());
                self.agent_color = Some(color.clone());

                // Start commit tick early so the spinner animates before the
                // first TextDelta/ThinkingDelta arrives.
                if !self.commit_tick_active {
                    self.commit_tick_active = true;
                }

                self.insert_agent_header(terminal, &agent_name, &display_name, &color)?;
            }
            AgentEvent::Retrying {
                attempt,
                max_attempts,
                reason,
                delay_secs,
            } => {
                self.agent_status_text = Some(format!(
                    "Retrying ({attempt}/{max_attempts}, {reason}, {delay_secs:.0}s)..."
                ));
            }
            AgentEvent::ThinkingDelta(text) => {
                tracing::debug!(delta = ?text, "ThinkingDelta received");

                // Clear retry status once content starts arriving.
                self.agent_status_text = None;

                if !self.is_thinking {
                    self.is_thinking = true;
                }

                // Use the same streaming pipeline but content will be
                // styled gray in insert_indented_lines_thinking.
                let collector = self
                    .stream_collector
                    .get_or_insert_with(MarkdownStreamCollector::new);
                collector.push_delta(&text);

                if collector.has_pending_newline() {
                    let lines = collector.commit_complete_lines();
                    if !lines.is_empty() {
                        self.stream_state.enqueue(lines);
                    }
                    if !self.commit_tick_active {
                        self.commit_tick_active = true;
                    }
                }
            }
            AgentEvent::TextDelta(text) => {
                tracing::debug!(delta = ?text, "TextDelta received");

                // Clear retry status once content starts arriving.
                self.agent_status_text = None;

                // Transition from thinking to text: finalize thinking content.
                if self.is_thinking {
                    self.finalize_thinking(terminal)?;
                }

                self.current_response_text.push_str(&text);

                // Push delta into markdown stream collector.
                let collector = self
                    .stream_collector
                    .get_or_insert_with(MarkdownStreamCollector::new);
                collector.push_delta(&text);

                // If we have pending newlines, commit and enqueue.
                if collector.has_pending_newline() {
                    let lines = collector.commit_complete_lines();
                    if !lines.is_empty() {
                        self.stream_state.enqueue(lines);
                    }

                    // Start commit tick animation on first content.
                    if !self.commit_tick_active {
                        self.commit_tick_active = true;
                    }
                }
            }
            AgentEvent::ToolCallStart { name, arguments } => {
                // Clear retry status.
                self.agent_status_text = None;

                // Finalize thinking if still active.
                if self.is_thinking {
                    self.finalize_thinking(terminal)?;
                }

                // Flush any buffered text content before tool line.
                if let Some(mut collector) = self.stream_collector.take() {
                    let remaining = collector.finalize();
                    if !remaining.is_empty() {
                        self.stream_state.enqueue(remaining);
                    }
                }
                let remaining_lines = self.stream_state.drain_all();
                if !remaining_lines.is_empty() {
                    self.insert_indented_lines(terminal, remaining_lines)?;
                }

                // Build tool call display line: ⚡ **tool_name**(args)
                let display = format_tool_call_display(&name, &arguments);
                let yellow = ratatui::style::Style::default().fg(ratatui::style::Color::Yellow);
                self.insert_tool_line(terminal, "\u{26A1} ", yellow, display)?;
            }
            AgentEvent::ToolCallDone {
                name: _,
                result_summary,
            } => {
                // Render result line below the tool call: ⎿  summary + blank line
                let dim = ratatui::style::Style::default().fg(ratatui::style::Color::DarkGray);
                self.insert_tool_line(
                    terminal,
                    "   \u{23BF}  ",
                    dim,
                    vec![ratatui::text::Span::raw(result_summary)],
                )?;
                terminal.insert_lines_above(vec![ratatui::text::Line::default()])?;
            }
            AgentEvent::Done {
                usage,
                intermediate_messages,
                final_text,
            } => {
                // Finalize thinking if still active.
                if self.is_thinking {
                    self.finalize_thinking(terminal)?;
                }

                // Finalize any remaining content in the collector.
                if let Some(mut collector) = self.stream_collector.take() {
                    let remaining = collector.finalize();
                    if !remaining.is_empty() {
                        self.stream_state.enqueue(remaining);
                    }
                }

                // Drain all remaining lines.
                let remaining_lines = self.stream_state.drain_all();
                if !remaining_lines.is_empty() {
                    self.insert_indented_lines(terminal, remaining_lines)?;
                }

                // Accumulate token usage for /agents display.
                if let Some(ref name) = self.current_agent_name {
                    let entry = self.agent_token_usage.entry(name.clone()).or_insert((0, 0));
                    entry.0 += usage.prompt_tokens;
                    entry.1 += usage.completion_tokens;
                }

                // Persist intermediate tool-round messages and final text.
                if let Some(agent_name) = self.current_agent_name.take() {
                    self.last_respondent = Some(agent_name.clone());
                    self.messages.extend(intermediate_messages);
                    self.messages.push(ChatMessage::text(
                        ChatRole::Assistant,
                        final_text,
                        Some(agent_name),
                    ));

                    // Persist session after agent response.
                    self.save_session();
                }

                // Clear accumulated streaming text (no longer used for message).
                self.current_response_text.clear();

                // Clear agent status indicator.
                self.agent_start_time = None;
                self.agent_display_name = None;
                self.agent_color = None;
                self.agent_status_text = None;

                // Reset streaming state for this agent.
                self.agent_event_rx = None;
                self.commit_tick_active = false;
                self.chunking_policy.reset();

                // Chain-trigger next pending agent (if any).
                self.start_next_agent(terminal)?;
            }
            AgentEvent::Error {
                message: msg,
                intermediate_messages,
            } => {
                // Finalize thinking if still active.
                if self.is_thinking {
                    self.finalize_thinking(terminal)?;
                }

                // Flush remaining buffered content to screen.
                if let Some(mut collector) = self.stream_collector.take() {
                    let remaining = collector.finalize();
                    if !remaining.is_empty() {
                        self.stream_state.enqueue(remaining);
                    }
                }
                let remaining_lines = self.stream_state.drain_all();
                if !remaining_lines.is_empty() {
                    self.insert_indented_lines(terminal, remaining_lines)?;
                }

                self.insert_agent_error(terminal, &msg)?;

                // Preserve intermediate tool-round messages collected before
                // the error, so they are not lost from session history.
                if let Some(agent_name) = self.current_agent_name.take() {
                    self.messages.extend(intermediate_messages);

                    // If the agent produced partial text output, preserve it
                    // with the error annotation.
                    if !self.current_response_text.is_empty() {
                        let mut content = std::mem::take(&mut self.current_response_text);
                        content.push_str(&format!("\n\n[Error: {msg}]"));
                        self.messages.push(ChatMessage::text(
                            ChatRole::Assistant,
                            content,
                            Some(agent_name),
                        ));
                    }
                }

                // Clear agent status indicator.
                self.agent_start_time = None;
                self.agent_display_name = None;
                self.agent_color = None;
                self.agent_status_text = None;

                // Reset streaming state.
                self.agent_event_rx = None;
                self.commit_tick_active = false;
                self.is_thinking = false;
                // Do NOT update last_respondent on error.
                self.current_response_text.clear();
                self.chunking_policy.reset();

                // Error isolation: continue with next pending agent.
                self.start_next_agent(terminal)?;
            }
        }

        Ok(())
    }

    /// Handle a commit tick: drain queued lines per adaptive chunking policy.
    fn handle_commit_tick(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let output = run_commit_tick(
            &mut self.chunking_policy,
            &mut self.stream_state,
            Instant::now(),
        );

        if !output.lines.is_empty() {
            // Use streaming variants (no trailing blank) — trailing blank is
            // added at the end of the response (Done event).
            if self.is_thinking {
                self.insert_thinking_lines_streaming(terminal, output.lines)?;
            } else {
                self.insert_indented_lines_streaming(terminal, output.lines)?;
            }
        }

        // Stop commit tick if idle and no active stream.
        if output.is_idle && self.agent_event_rx.is_none() {
            self.commit_tick_active = false;
        }

        Ok(())
    }

    /// Finalize the thinking phase: drain remaining thinking lines and reset.
    fn finalize_thinking(
        &mut self,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        // Finalize the thinking collector.
        if let Some(mut collector) = self.stream_collector.take() {
            let remaining = collector.finalize();
            if !remaining.is_empty() {
                self.stream_state.enqueue(remaining);
            }
        }

        // Drain all remaining thinking lines.
        let remaining = self.stream_state.drain_all();
        if !remaining.is_empty() {
            self.insert_thinking_lines(terminal, remaining)?;
        }

        self.is_thinking = false;
        self.chunking_policy.reset();
        Ok(())
    }

    /// Request a redraw via the frame scheduler.
    fn request_redraw(&self) {
        if let Some(fr) = &self.frame_requester {
            fr.schedule_frame();
        }
    }

    /// Handle paste burst tick during draw. Returns true to skip rendering.
    fn handle_paste_burst_tick(&mut self) -> bool {
        if !self.config.settings.paste_burst_detection {
            return false;
        }
        // Try flushing timed-out burst.
        let flushed = match self.paste_burst.flush_if_due(Instant::now()) {
            FlushResult::Paste(p) => {
                self.handle_paste(p);
                true
            }
            FlushResult::Typed(c) => {
                self.textarea.insert_str(c.to_string().as_str());
                true
            }
            FlushResult::None => false,
        };
        if flushed {
            self.request_redraw();
            return true;
        }
        if self.paste_burst.is_active() {
            // Still buffering — schedule follow-up tick, skip render.
            if let Some(fr) = &self.frame_requester {
                fr.schedule_frame_in(self.paste_burst.recommended_flush_delay());
            }
            return true;
        }
        false
    }

    /// Handle a single terminal event.
    fn handle_event(
        &mut self,
        event: Event,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        match event {
            Event::Key(key_event) => {
                if key_event.kind != KeyEventKind::Press {
                    return Ok(());
                }
                self.handle_key(key_event, terminal)?;
            }
            Event::Paste(text) => {
                // Receiving Event::Paste means the terminal supports
                // bracketed paste — auto-disable burst detection.
                if self.config.settings.paste_burst_detection {
                    tracing::info!("Bracketed paste detected, disabling paste burst detection");
                    self.config.settings.paste_burst_detection = false;
                }
                self.handle_paste(text);
            }
            Event::Resize(..) => {}
            _ => {}
        }
        Ok(())
    }

    /// Threshold in chars above which pasted text is collapsed into a
    /// placeholder element.
    const PASTE_PLACEHOLDER_THRESHOLD: usize = 100;

    /// Handle a paste event (bracketed paste or burst-detected paste) —
    /// insert text into the textarea without triggering auto-send on newlines.
    pub(crate) fn handle_paste(&mut self, text: String) {
        self.paste_burst.clear_after_explicit_paste();
        let text = text.replace("\r\n", "\n").replace('\r', "\n");

        if text.chars().count() > Self::PASTE_PLACEHOLDER_THRESHOLD {
            self.paste_counter += 1;
            let n = self.paste_counter;
            let char_count = text.chars().count();
            let placeholder = format!("[Pasted text #{n} ({char_count} chars)]");
            let elem_id = self.textarea.insert_element(&placeholder);
            self.pending_pastes.insert(elem_id, text);
        } else {
            self.textarea.insert_str(&text);
        }
    }

    /// Clear the textarea and any pending paste placeholders.
    pub(crate) fn clear_textarea(&mut self) {
        self.textarea = TextArea::new();
        self.history_index = None;
        self.pending_pastes.clear();
        self.paste_counter = 0;
    }

    /// Return the textarea text with paste placeholders expanded to
    /// their actual pasted content.
    pub(crate) fn expanded_text(&self) -> String {
        if self.pending_pastes.is_empty() {
            return self.textarea.text().to_string();
        }

        let mut result = self.textarea.text().to_string();
        // Expand in reverse order of element position so that earlier
        // replacements don't shift later byte ranges.
        let mut replacements: Vec<_> = self
            .textarea
            .elements_snapshot()
            .into_iter()
            .filter_map(|(id, range)| {
                self.pending_pastes
                    .get(&id)
                    .map(|real| (range, real.clone()))
            })
            .collect();
        replacements.sort_by(|a, b| b.0.start.cmp(&a.0.start));

        for (range, real_text) in replacements {
            if range.end <= result.len() {
                result.replace_range(range, &real_text);
            }
        }
        result
    }

    /// Replace textarea content with the given text (supports multiline).
    pub(crate) fn set_textarea_content(&mut self, content: &str) {
        self.textarea.set_text_clearing_elements(content);
        self.textarea.set_cursor(content.len());
    }
}
