//! App state machine and main event loop.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossterm::event::{Event, EventStream, KeyEventKind};
use futures::StreamExt;
use tokio::sync::{Notify, mpsc};

use krew_config::Config;
use krew_core::agent::AgentRuntime;
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

    // --- Agent status indicator ---
    /// Timestamp when the current agent started processing (drives status line visibility).
    pub agent_start_time: Option<Instant>,
    /// Display name of the currently active agent (shown in status line).
    pub agent_display_name: Option<String>,
    /// Color name of the currently active agent (shown in status line).
    pub agent_color: Option<String>,
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

        // Initialize agent runtimes.
        let agents = Self::init_agents(&config);

        Ok(Self {
            cwd,
            config,
            project_instructions,
            textarea: TextArea::new(),
            should_quit: false,
            quit_shortcut_armed_at: None,
            quit_hint: None,
            popup: ActivePopup::None,
            history: Vec::new(),
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
            startup_warnings: Vec::new(),
            agent_start_time: None,
            agent_display_name: None,
            agent_color: None,
        })
    }

    /// Build AgentRuntime instances from config.
    fn init_agents(config: &Config) -> HashMap<String, AgentRuntime> {
        let mut agents = HashMap::new();

        for agent_config in &config.agents {
            if agent_config.provider == "builtin" {
                // Skip builtin echo agents — they don't need an LLM client.
                continue;
            }

            let provider_config = match config.providers.get(&agent_config.provider) {
                Some(p) => p,
                None => {
                    tracing::warn!(
                        agent = agent_config.name,
                        provider = agent_config.provider,
                        "Provider not found, skipping agent"
                    );
                    continue;
                }
            };

            // Determine API key env var.
            let api_key_env = match &provider_config.api_key_env {
                Some(env) => env.as_str(),
                None => {
                    tracing::warn!(
                        agent = agent_config.name,
                        "No api_key_env configured, skipping agent"
                    );
                    continue;
                }
            };

            // Create LLM client based on provider type.
            let client: Arc<dyn krew_llm::LlmClient> = match provider_config.provider_type {
                krew_config::ProviderType::OpenAI => {
                    let api_type = agent_config.api_type.unwrap_or(krew_config::ApiType::Chat);
                    match api_type {
                        krew_config::ApiType::Chat => {
                            match krew_llm::openai_chat::OpenAiChatClient::new(
                                agent_config.name.clone(),
                                agent_config.model.clone(),
                                api_key_env,
                                provider_config.base_url.as_deref(),
                                provider_config.use_name_field,
                            ) {
                                Ok(c) => Arc::new(c),
                                Err(e) => {
                                    tracing::warn!(
                                        agent = agent_config.name,
                                        error = %e,
                                        "Failed to create LLM client, skipping agent"
                                    );
                                    continue;
                                }
                            }
                        }
                        krew_config::ApiType::Responses => {
                            match krew_llm::OpenAiResponsesClient::new(
                                agent_config.name.clone(),
                                agent_config.model.clone(),
                                api_key_env,
                                provider_config.base_url.as_deref(),
                                agent_config.enable_thinking,
                                agent_config.thinking_effort,
                            ) {
                                Ok(c) => Arc::new(c),
                                Err(e) => {
                                    tracing::warn!(
                                        agent = agent_config.name,
                                        error = %e,
                                        "Failed to create LLM client, skipping agent"
                                    );
                                    continue;
                                }
                            }
                        }
                    }
                }
                other => {
                    tracing::warn!(
                        agent = agent_config.name,
                        provider_type = ?other,
                        "Provider type not yet supported, skipping agent"
                    );
                    continue;
                }
            };

            let runtime = AgentRuntime {
                config: agent_config.clone(),
                client,
                tools: Vec::new(),
                is_responding: false,
                use_name_field: provider_config.use_name_field,
            };

            agents.insert(agent_config.name.clone(), runtime);
        }

        agents
    }

    /// Run the main event loop.
    pub async fn run(&mut self, terminal: &mut custom_terminal::Terminal) -> anyhow::Result<()> {
        // Print the header above the viewport (scrolls into scrollback).
        render::insert_header(terminal, self)?;

        // Display startup warnings.
        for warning in std::mem::take(&mut self.startup_warnings) {
            self.show_warning(terminal, &warning)?;
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
            AgentEvent::ThinkingDelta(text) => {
                tracing::debug!(delta = ?text, "ThinkingDelta received");

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
            AgentEvent::Done(usage) => {
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

                // Add assistant message to history.
                if let Some(agent_name) = self.current_agent_name.take() {
                    self.messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: std::mem::take(&mut self.current_response_text),
                        name: Some(agent_name),
                    });
                }

                // Clear agent status indicator.
                self.agent_start_time = None;
                self.agent_display_name = None;
                self.agent_color = None;

                // Reset streaming state.
                self.agent_event_rx = None;
                self.commit_tick_active = false;
                self.chunking_policy.reset();
            }
            AgentEvent::Error(msg) => {
                self.insert_agent_error(terminal, &msg)?;

                // Clear agent status indicator.
                self.agent_start_time = None;
                self.agent_display_name = None;
                self.agent_color = None;

                // Reset streaming state.
                self.stream_collector = None;
                self.agent_event_rx = None;
                self.commit_tick_active = false;
                self.is_thinking = false;
                self.current_agent_name = None;
                self.current_response_text.clear();
                self.chunking_policy.reset();
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
            if self.is_thinking {
                self.insert_thinking_lines(terminal, output.lines)?;
            } else {
                self.insert_indented_lines(terminal, output.lines)?;
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
