//! Approval overlay widget.
//!
//! Renders a modal selection UI when a tool requires user approval.
//! The overlay intercepts keyboard events and sends the user's decision
//! back to the agent loop via a oneshot channel.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use krew_core::event::ReviewDecision;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Widget};
use tokio::sync::oneshot;

/// A single approval option with display label and keyboard shortcut.
struct ApprovalOption {
    /// Display label (e.g. "Yes, proceed").
    label: String,
    /// Keyboard shortcut character (e.g. 'y').
    shortcut: char,
    /// The decision this option represents.
    decision: ReviewDecision,
}

/// Pending approval request waiting for user decision.
struct PendingRequest {
    /// Tool name (e.g. "shell", "edit_file").
    tool_name: String,
    /// Raw JSON arguments string.
    arguments: String,
    /// Whether the "Approve for Session" option should be shown.
    allow_session_approval: bool,
    /// Human-readable scope for session approval (e.g. "cargo build", "edit_file").
    session_scope: String,
    /// Channel to send the decision back to the agent loop.
    respond: oneshot::Sender<ReviewDecision>,
}

/// Modal overlay for tool approval decisions.
///
/// Manages a queue of approval requests, displaying one at a time.
/// Each request shows the tool name, arguments, and selectable options
/// with keyboard shortcuts.
pub struct ApprovalOverlay {
    /// Currently displayed request.
    current: Option<PendingRequest>,
    /// Queue of additional requests waiting.
    queue: Vec<PendingRequest>,
    /// Available options for the current request.
    options: Vec<ApprovalOption>,
    /// Currently selected option index.
    selected: usize,
    /// Whether the overlay has been fully dismissed.
    done: bool,
}

impl ApprovalOverlay {
    /// Create a new overlay with the first approval request.
    pub fn new(
        tool_name: String,
        arguments: String,
        allow_session_approval: bool,
        respond: oneshot::Sender<ReviewDecision>,
    ) -> Self {
        let session_scope = compute_session_scope(&tool_name, &arguments);
        let options = build_options(allow_session_approval, &session_scope);
        Self {
            current: Some(PendingRequest {
                tool_name,
                arguments,
                allow_session_approval,
                session_scope,
                respond,
            }),
            queue: Vec::new(),
            options,
            selected: 0,
            done: false,
        }
    }

    /// Enqueue an additional approval request.
    pub fn enqueue(
        &mut self,
        tool_name: String,
        arguments: String,
        allow_session_approval: bool,
        respond: oneshot::Sender<ReviewDecision>,
    ) {
        let session_scope = compute_session_scope(&tool_name, &arguments);
        self.queue.push(PendingRequest {
            tool_name,
            arguments,
            allow_session_approval,
            session_scope,
            respond,
        });
    }

    /// Whether the overlay is fully done (no more requests).
    pub fn is_done(&self) -> bool {
        self.done
    }

    /// Handle a key event.
    ///
    /// Returns `Some(decision)` when the user makes a choice,
    /// `None` when the event was navigation or unrecognized.
    pub fn handle_key(&mut self, key: KeyEvent) -> Option<ReviewDecision> {
        // Ctrl+C aborts the current request and clears the queue.
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.send_decision(ReviewDecision::Abort);
            for req in self.queue.drain(..) {
                let _ = req.respond.send(ReviewDecision::Abort);
            }
            self.done = true;
            return Some(ReviewDecision::Abort);
        }

        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                } else {
                    self.selected = self.options.len().saturating_sub(1);
                }
                None
            }
            KeyCode::Down => {
                if self.selected + 1 < self.options.len() {
                    self.selected += 1;
                } else {
                    self.selected = 0;
                }
                None
            }
            KeyCode::Enter => {
                let decision = self
                    .options
                    .get(self.selected)
                    .map(|o| o.decision.clone())?;
                self.send_decision(decision.clone());
                self.advance_queue();
                Some(decision)
            }
            KeyCode::Esc => {
                self.send_decision(ReviewDecision::Denied);
                self.advance_queue();
                Some(ReviewDecision::Denied)
            }
            KeyCode::Char(ch) => {
                let decision = self
                    .options
                    .iter()
                    .find(|o| o.shortcut == ch)
                    .map(|o| o.decision.clone())?;
                self.send_decision(decision.clone());
                self.advance_queue();
                Some(decision)
            }
            _ => None,
        }
    }

    /// Render the overlay into ratatui Lines for display.
    ///
    /// Returns lines that should be rendered in the viewport area.
    pub fn render_lines(&self) -> Vec<Line<'static>> {
        let Some(req) = &self.current else {
            return vec![];
        };

        let mut lines: Vec<Line<'static>> = Vec::new();

        // Blank line before.
        lines.push(Line::default());

        // Header: tool call info.
        let tool_display = format_tool_display(&req.tool_name, &req.arguments);
        lines.push(Line::from(vec![
            Span::styled("  \u{26A1} ", Style::default().fg(Color::Yellow)),
            Span::styled(tool_display, Style::default().bold()),
            Span::styled(" — approve?", Style::default().fg(Color::DarkGray)),
        ]));

        lines.push(Line::default());

        // Options list with selection indicator.
        for (i, option) in self.options.iter().enumerate() {
            let is_selected = i == self.selected;
            let indicator = if is_selected { " \u{203A} " } else { "   " };
            let shortcut_display = format!("({})", option.shortcut);

            let style = if is_selected {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };

            lines.push(Line::from(vec![
                Span::styled(indicator, style),
                Span::styled(option.label.clone(), style),
                Span::raw("  "),
                Span::styled(shortcut_display, Style::default().fg(Color::DarkGray)),
            ]));
        }

        lines.push(Line::default());

        // Footer hint.
        lines.push(Line::from(Span::styled(
            "  Press shortcut key or Enter to confirm, Esc to deny",
            Style::default().fg(Color::DarkGray),
        )));

        lines
    }

    /// Render as a ratatui widget into a buffer area.
    pub fn render_widget(&self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let lines = self.render_lines();
        let block = Block::default()
            .borders(Borders::TOP)
            .border_style(Style::default().fg(Color::DarkGray));
        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }

    /// Number of lines needed to render the overlay.
    pub fn desired_height(&self) -> u16 {
        self.render_lines().len() as u16 + 1 // +1 for the border
    }

    // -- Internal helpers -------------------------------------------------

    /// Send a decision for the current request.
    fn send_decision(&mut self, decision: ReviewDecision) {
        if let Some(req) = self.current.take() {
            let _ = req.respond.send(decision);
        }
    }

    /// Advance to the next queued request, or mark as done.
    fn advance_queue(&mut self) {
        if let Some(next) = self.queue.pop() {
            let allow_session = next.allow_session_approval;
            let scope = next.session_scope.clone();
            self.current = Some(next);
            self.selected = 0;
            self.options = build_options(allow_session, &scope);
        } else {
            self.done = true;
        }
    }
}

/// Build the approval options.
///
/// Options:
/// - y: Approve this time
/// - a: Approve for session (only if `allow_session_approval` is true)
/// - n: Deny
/// - Esc: Deny (alternative)
fn build_options(allow_session_approval: bool, session_scope: &str) -> Vec<ApprovalOption> {
    let mut options = vec![ApprovalOption {
        label: "Yes, proceed".to_string(),
        shortcut: 'y',
        decision: ReviewDecision::Approved,
    }];

    if allow_session_approval {
        options.push(ApprovalOption {
            label: format!("Yes, always approve \"{session_scope}\" this session"),
            shortcut: 'a',
            decision: ReviewDecision::ApprovedForSession,
        });
    }

    options.push(ApprovalOption {
        label: "No, skip this tool".to_string(),
        shortcut: 'n',
        decision: ReviewDecision::Denied,
    });

    options
}

/// Compute the human-readable session approval scope for a tool call.
///
/// For shell tools, extracts command prefixes via `extract_command_prefixes`
/// (e.g. "cargo build"). For other tools, returns the tool name.
fn compute_session_scope(tool_name: &str, arguments: &str) -> String {
    if tool_name == "shell"
        && let Ok(args) = serde_json::from_str::<serde_json::Value>(arguments)
        && let Some(command) = args.get("command").and_then(|c| c.as_str())
        && let Some(mut prefixes) = krew_tools::builtin::extract_command_prefixes(command)
    {
        prefixes.dedup();
        return prefixes.join(", ");
    }
    tool_name.to_string()
}

/// Parameters rendered as diff previews rather than inline arguments.
fn is_content_param(tool_name: &str, param_name: &str) -> bool {
    matches!(
        (tool_name, param_name),
        ("write_file", "content") | ("edit_file", "old_string") | ("edit_file", "new_string")
    )
}

/// Format tool name and arguments for display in the approval header.
fn format_tool_display(tool_name: &str, arguments: &str) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    let params = match args.as_object() {
        Some(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .filter(|(key, _)| !is_content_param(tool_name, key))
                .map(|(key, val)| {
                    let display = match val {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        other => other.to_string(),
                    };
                    if obj.keys().find(|k| !is_content_param(tool_name, k)) == Some(key) {
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
    format!("{tool_name}({params})")
}
