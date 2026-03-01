//! Keyboard input handling, completion popup interaction, and input history.

use std::time::Instant;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::{Color, Modifier, Style};
use ratatui_textarea::{Input, Key, TextArea};

use krew_core::command::SlashCommand;

use crate::completion::{ActivePopup, CompletionItem, CompletionState};
use crate::custom_terminal;

use super::App;
use super::paste_burst::CharDecision;

impl<'a> App<'a> {
    /// Handle a key press event.
    pub(crate) fn handle_key(
        &mut self,
        key_event: KeyEvent,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<()> {
        let now = Instant::now();
        let burst_enabled = self.config.settings.paste_burst_detection;

        // Flush any due paste burst before handling a new key.
        if burst_enabled {
            self.flush_paste_burst_now(now);
        }

        // Ctrl+C: double-press to quit.
        if key_event.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key_event.code, KeyCode::Char('c') | KeyCode::Char('C'))
        {
            if burst_enabled {
                self.flush_and_apply_burst();
                self.paste_burst.clear_window_after_non_char();
            }
            self.on_ctrl_c();
            return Ok(());
        }

        // Clear any quit hint on other key presses.
        if self.quit_hint.is_some() {
            self.quit_shortcut_armed_at = None;
            self.quit_hint = None;
        }

        // If popup is active, intercept navigation keys.
        if self.popup.is_active() && self.handle_popup_key(key_event, terminal)? {
            if burst_enabled {
                self.flush_and_apply_burst();
                self.paste_burst.clear_window_after_non_char();
            }
            return Ok(());
        }

        // --- Paste burst detection (when enabled) ---
        if burst_enabled {
            // If capturing a burst and Enter is pressed, buffer it as newline.
            if matches!(key_event.code, KeyCode::Enter)
                && self.paste_burst.is_active()
                && self.paste_burst.append_newline_if_active(now)
            {
                return Ok(());
            }

            // During the burst enter-suppress window, treat Enter as newline.
            if matches!(key_event.code, KeyCode::Enter)
                && !key_event.modifiers.contains(KeyModifiers::SHIFT)
                && !key_event.modifiers.contains(KeyModifiers::CONTROL)
                && self
                    .paste_burst
                    .newline_should_insert_instead_of_submit(now)
            {
                self.textarea.insert_newline();
                self.paste_burst.extend_window(now);
                return Ok(());
            }

            // Intercept plain chars (no Ctrl/Alt) for burst detection.
            if let KeyCode::Char(ch) = key_event.code {
                let has_ctrl_or_alt = key_event.modifiers.contains(KeyModifiers::CONTROL)
                    || key_event.modifiers.contains(KeyModifiers::ALT);

                if !has_ctrl_or_alt {
                    if !ch.is_ascii() {
                        return self.handle_non_ascii_char(key_event, ch, now);
                    }

                    match self.paste_burst.on_plain_char(ch, now) {
                        CharDecision::BufferAppend => {
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return Ok(());
                        }
                        CharDecision::BeginBuffer { retro_chars } => {
                            let lines = self.textarea.lines();
                            let (row, col) = self.textarea.cursor();
                            if let Some(line) = lines.get(row) {
                                let byte_col = Self::char_to_byte(line, col);
                                let before = &line[..byte_col];
                                if self
                                    .paste_burst
                                    .decide_begin_buffer(now, before, retro_chars as usize)
                                    .is_some()
                                {
                                    self.paste_burst.append_char_to_buffer(ch, now);
                                    return Ok(());
                                }
                            }
                            // Not paste-like enough, fall through to normal insertion.
                        }
                        CharDecision::BeginBufferFromPending => {
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return Ok(());
                        }
                        CharDecision::RetainFirstChar => {
                            return Ok(());
                        }
                    }
                }
                // Flush burst before applying a modified char (Ctrl/Alt+char).
                self.flush_and_apply_burst();
            }
        }

        // Flush any buffered burst before applying non-char input.
        if burst_enabled && !matches!(key_event.code, KeyCode::Char(_) | KeyCode::Enter) {
            self.flush_and_apply_burst();
        }

        let input: Input = key_event.into();
        match input {
            // Enter (no modifiers) => send message.
            Input {
                key: Key::Enter,
                shift: false,
                ctrl: false,
                alt: false,
            } => {
                self.send_message(terminal)?;
            }
            // Shift+Enter => insert newline.
            Input {
                key: Key::Enter,
                shift: true,
                ..
            } => {
                self.textarea.insert_newline();
            }
            // Ctrl+J => insert newline (fallback for terminals without
            // keyboard enhancement where Shift+Enter is indistinguishable
            // from Enter).
            Input {
                key: Key::Char('j'),
                ctrl: true,
                shift: false,
                alt: false,
            } => {
                self.textarea.insert_newline();
            }
            // Up arrow: history prev when cursor is on the first row.
            Input {
                key: Key::Up,
                shift: false,
                ctrl: false,
                alt: false,
            } if self.textarea.cursor().0 == 0 => {
                self.history_prev();
            }
            // Down arrow: history next when cursor is on the last row.
            Input {
                key: Key::Down,
                shift: false,
                ctrl: false,
                alt: false,
            } if self.textarea.cursor().0 == self.textarea.lines().len() - 1 => {
                self.history_next();
            }
            // All other keys => forward to textarea.
            other => {
                self.textarea.input(other);
            }
        }

        // Update paste burst state after processing.
        if burst_enabled {
            match key_event.code {
                KeyCode::Char(_) => {
                    let has_ctrl_or_alt = key_event.modifiers.contains(KeyModifiers::CONTROL)
                        || key_event.modifiers.contains(KeyModifiers::ALT);
                    if has_ctrl_or_alt {
                        self.paste_burst.clear_window_after_non_char();
                    }
                }
                KeyCode::Enter => {
                    // Keep burst window alive (supports blank lines in paste).
                }
                _ => {
                    self.paste_burst.clear_window_after_non_char();
                }
            }
        }

        Ok(())
    }

    /// Handle non-ASCII character input (often IME) with burst detection.
    fn handle_non_ascii_char(
        &mut self,
        key_event: KeyEvent,
        ch: char,
        now: Instant,
    ) -> anyhow::Result<()> {
        // If already in a burst, capture directly.
        if self.paste_burst.try_append_char_if_active(ch, now) {
            return Ok(());
        }

        // Flush any existing burst state before checking.
        self.flush_and_apply_burst();

        if let Some(decision) = self.paste_burst.on_plain_char_no_hold(now) {
            match decision {
                CharDecision::BufferAppend => {
                    self.paste_burst.append_char_to_buffer(ch, now);
                    return Ok(());
                }
                CharDecision::BeginBuffer { retro_chars } => {
                    let lines = self.textarea.lines();
                    let (row, col) = self.textarea.cursor();
                    if let Some(line) = lines.get(row) {
                        let byte_col = Self::char_to_byte(line, col);
                        let before = &line[..byte_col];
                        if self
                            .paste_burst
                            .decide_begin_buffer(now, before, retro_chars as usize)
                            .is_some()
                        {
                            self.paste_burst.append_char_to_buffer(ch, now);
                            return Ok(());
                        }
                    }
                    // Retro-grab rejected (e.g. short CJK text without
                    // whitespace), but the input speed is paste-like.
                    // Start buffering from this point without retro-capture.
                    self.paste_burst.force_start_buffer(now);
                    self.paste_burst.append_char_to_buffer(ch, now);
                    return Ok(());
                }
                _ => {}
            }
        }

        // Normal insertion for non-ASCII chars.
        self.flush_and_apply_burst();
        self.textarea.input(Input::from(key_event));
        Ok(())
    }

    /// Flush any buffered paste burst and apply it via handle_paste.
    fn flush_and_apply_burst(&mut self) {
        if let Some(pasted) = self.paste_burst.flush_before_modified_input() {
            self.handle_paste(pasted);
        }
    }

    /// Flush a paste burst at a specific time (called from tick and handle_key).
    fn flush_paste_burst_now(&mut self, now: Instant) {
        match self.paste_burst.flush_if_due(now) {
            super::paste_burst::FlushResult::Paste(pasted) => {
                self.handle_paste(pasted);
            }
            super::paste_burst::FlushResult::Typed(ch) => {
                self.textarea.insert_str(ch.to_string().as_str());
            }
            super::paste_burst::FlushResult::None => {}
        }
    }

    /// Handle Ctrl+C press with double-press detection.
    fn on_ctrl_c(&mut self) {
        if let Some(armed_at) = self.quit_shortcut_armed_at
            && armed_at.elapsed() < super::QUIT_SHORTCUT_TIMEOUT
        {
            self.should_quit = true;
            return;
        }

        self.quit_shortcut_armed_at = Some(std::time::Instant::now());
        self.quit_hint = Some("Press Ctrl+C again to quit".to_string());
    }

    // ── Completion popup ─────────────────────────────────────────────

    /// Handle keys when popup is active. Returns true if the key was consumed.
    fn handle_popup_key(
        &mut self,
        key_event: KeyEvent,
        terminal: &mut custom_terminal::Terminal,
    ) -> anyhow::Result<bool> {
        match key_event.code {
            KeyCode::Up => {
                match &mut self.popup {
                    ActivePopup::SlashCommand(s) | ActivePopup::AgentName(s) => s.move_up(),
                    ActivePopup::None => {}
                }
                Ok(true)
            }
            KeyCode::Down => {
                match &mut self.popup {
                    ActivePopup::SlashCommand(s) | ActivePopup::AgentName(s) => s.move_down(),
                    ActivePopup::None => {}
                }
                Ok(true)
            }
            KeyCode::Tab => {
                self.accept_completion();
                Ok(true)
            }
            KeyCode::Enter => {
                // For slash commands, execute directly. For agent names, insert.
                match &self.popup {
                    ActivePopup::SlashCommand(state) => {
                        if let Some(item) = state.selected_item() {
                            let cmd_input = item.value.clone();
                            self.popup = ActivePopup::None;
                            self.clear_textarea();
                            self.execute_slash_command(&cmd_input, terminal)?;
                            return Ok(true);
                        }
                    }
                    ActivePopup::AgentName(_) => {
                        self.accept_completion();
                        return Ok(true);
                    }
                    ActivePopup::None => {}
                }
                Ok(false)
            }
            KeyCode::Esc => {
                self.popup = ActivePopup::None;
                Ok(true)
            }
            _ => Ok(false), // Let other keys pass through to textarea.
        }
    }

    /// Accept the currently selected completion item.
    fn accept_completion(&mut self) {
        match &self.popup {
            ActivePopup::SlashCommand(state) => {
                if let Some(item) = state.selected_item() {
                    // Replace entire input with the slash command + trailing space.
                    let text = format!("{} ", item.value);
                    self.set_textarea_content(&text);
                }
            }
            ActivePopup::AgentName(state) => {
                if let Some(item) = state.selected_item() {
                    let insert_text = format!("@{} ", item.value);
                    // Replace the @token at cursor with the selected name.
                    self.replace_at_token(&insert_text);
                }
            }
            ActivePopup::None => {}
        }
        self.popup = ActivePopup::None;
    }

    /// Replace the `@token` at the cursor position with `replacement`.
    fn replace_at_token(&mut self, replacement: &str) {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let Some(line) = lines.get(row) else {
            return;
        };
        let line = line.clone();
        let byte_col = Self::char_to_byte(&line, col);

        // Find the @token boundaries around the cursor.
        let before = &line[..byte_col];
        let at_pos = match before.rfind('@') {
            Some(pos) => pos,
            None => return,
        };

        // Find the end of the token (next whitespace or end of line).
        let after_at = &line[at_pos..];
        let token_end = after_at
            .find(|c: char| c.is_whitespace())
            .map(|i| at_pos + i)
            .unwrap_or(line.len());

        // Build new line.
        let new_line = format!("{}{}{}", &line[..at_pos], replacement, &line[token_end..]);

        // Rebuild all lines — cursor position is in characters, not bytes.
        let new_col_chars = line[..at_pos].chars().count() + replacement.chars().count();

        let mut all_lines: Vec<String> = lines.to_vec();
        all_lines[row] = new_line.clone();

        let mut textarea = TextArea::new(all_lines);
        textarea.set_cursor_line_style(Style::default());
        textarea.set_cursor_style(
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::REVERSED),
        );
        // Move cursor to the correct position.
        for _ in 0..row {
            textarea.move_cursor(ratatui_textarea::CursorMove::Down);
        }
        textarea.move_cursor(ratatui_textarea::CursorMove::Head);
        for _ in 0..new_col_chars {
            textarea.move_cursor(ratatui_textarea::CursorMove::Forward);
        }
        self.textarea = textarea;
    }

    // ── Completion popup sync ────────────────────────────────────────

    /// Detect whether a completion popup should be shown based on current input.
    pub(crate) fn sync_popup(&mut self) {
        let lines = self.textarea.lines();
        let first_line = lines.first().map(|s| s.as_str()).unwrap_or("");

        // Slash command popup: first line starts with `/`.
        if first_line.starts_with('/') && lines.len() == 1 {
            let filter = first_line; // include `/` so it matches "/agents" etc.
            match &mut self.popup {
                ActivePopup::SlashCommand(state) => {
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    }
                }
                _ => {
                    let mut state = CompletionState::new(self.slash_command_items());
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    } else {
                        self.popup = ActivePopup::SlashCommand(state);
                    }
                }
            }
            return;
        }

        // Agent name popup: detect @token at cursor position.
        if let Some(token) = self.current_at_token() {
            let filter = &token; // text after `@`
            match &mut self.popup {
                ActivePopup::AgentName(state) => {
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    }
                }
                _ => {
                    let mut state = CompletionState::new(self.agent_name_items());
                    state.set_filter(filter);
                    if state.is_empty() {
                        self.popup = ActivePopup::None;
                    } else {
                        self.popup = ActivePopup::AgentName(state);
                    }
                }
            }
            return;
        }

        // No trigger condition — close popup.
        self.popup = ActivePopup::None;
    }

    /// Convert a character index to a byte offset within `s`.
    /// Returns `s.len()` if `char_idx` is beyond the string.
    fn char_to_byte(s: &str, char_idx: usize) -> usize {
        s.char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(s.len())
    }

    /// Extract the `@token` at the current cursor position, if any.
    /// Returns the text after `@` (may be empty for bare `@`).
    fn current_at_token(&self) -> Option<String> {
        let (row, col) = self.textarea.cursor();
        let lines = self.textarea.lines();
        let line = lines.get(row)?;
        let byte_col = Self::char_to_byte(line, col);
        let before = &line[..byte_col];

        // Find the last `@` not preceded by a non-whitespace char.
        let at_pos = before.rfind('@')?;
        // Check that `@` is at start or preceded by whitespace.
        if at_pos > 0 {
            let prev_char = line[..at_pos].chars().next_back()?;
            if !prev_char.is_whitespace() {
                return None;
            }
        }

        // Extract the token from @ to the next whitespace (or cursor).
        let token_start = at_pos + 1; // '@' is 1 byte
        let token = &line[token_start..byte_col];

        // Don't trigger if there's a space between @ and cursor.
        if token.contains(' ') {
            return None;
        }

        Some(token.to_string())
    }

    /// Build completion items for slash commands.
    fn slash_command_items(&self) -> Vec<CompletionItem> {
        SlashCommand::all_help()
            .iter()
            .map(|&(name, desc)| CompletionItem {
                value: name.to_string(),
                description: desc.to_string(),
            })
            .collect()
    }

    /// Build completion items for agent names (including "all").
    fn agent_name_items(&self) -> Vec<CompletionItem> {
        let mut items = vec![CompletionItem {
            value: "all".to_string(),
            description: "Broadcast to all agents".to_string(),
        }];
        for agent in &self.config.agents {
            items.push(CompletionItem {
                value: agent.name.clone(),
                description: format!(
                    "{} ({}/{})",
                    agent.display_name, agent.provider, agent.model
                ),
            });
        }
        items
    }

    // ── Input history ────────────────────────────────────────────────

    /// Push an entry to the input history, respecting the configured limit.
    pub(crate) fn history_push(&mut self, entry: String) {
        // Avoid consecutive duplicates.
        if self.history.last().is_some_and(|last| last == &entry) {
            return;
        }
        self.history.push(entry);
        let limit = self.config.settings.input_history_limit;
        if self.history.len() > limit {
            self.history.drain(..self.history.len() - limit);
        }
    }

    /// Navigate to the previous history entry (Up arrow).
    fn history_prev(&mut self) {
        if self.history.is_empty() {
            return;
        }
        let idx = match self.history_index {
            Some(i) if i > 0 => i - 1,
            Some(_) => return, // already at oldest
            None => {
                // Entering history — save current input as draft.
                self.history_draft = self.textarea.lines().join("\n");
                self.history.len() - 1
            }
        };
        self.history_index = Some(idx);
        self.load_history_entry(idx);
    }

    /// Navigate to the next history entry (Down arrow).
    fn history_next(&mut self) {
        let Some(current) = self.history_index else {
            return; // not browsing history
        };
        if current + 1 < self.history.len() {
            let idx = current + 1;
            self.history_index = Some(idx);
            self.load_history_entry(idx);
        } else {
            // Past the newest entry — restore draft.
            self.history_index = None;
            self.set_textarea_content(&self.history_draft.clone());
        }
    }

    /// Load a history entry into the textarea.
    fn load_history_entry(&mut self, idx: usize) {
        if let Some(entry) = self.history.get(idx) {
            self.set_textarea_content(&entry.clone());
        }
    }
}
