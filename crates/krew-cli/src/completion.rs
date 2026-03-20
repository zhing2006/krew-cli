//! Completion popup for slash commands and agent names.
//!
//! The popup replaces the status bar area when active and expands the
//! viewport height downward (via `ensure_viewport_height`), pushing
//! content above upward.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

/// Maximum number of visible rows in the completion popup.
pub const MAX_POPUP_ROWS: usize = 8;

/// Which completion popup is currently active.
#[derive(Debug)]
pub enum ActivePopup {
    /// No popup visible.
    None,
    /// Slash command completion (triggered by `/` at start of first line).
    SlashCommand(CompletionState),
    /// Agent name completion (triggered by `@` token at cursor).
    AgentName(CompletionState),
    /// Whisper target completion (triggered by `#` token at cursor).
    WhisperName(CompletionState),
    /// Session picker (triggered by `/resume`).
    SessionPicker(CompletionState),
}

/// Completion item with display text and optional description.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert on selection.
    pub value: String,
    /// Description shown next to the value in the popup.
    pub description: String,
}

/// State for an active completion popup.
#[derive(Debug)]
pub struct CompletionState {
    /// All candidate items (unfiltered).
    all_items: Vec<CompletionItem>,
    /// Indices into `all_items` that match the current filter.
    filtered: Vec<usize>,
    /// Currently selected index within `filtered`.
    pub selected: usize,
    /// Current filter text (without the trigger character).
    pub filter: String,
}

impl CompletionState {
    /// Create a new completion state with the given candidates.
    pub fn new(items: Vec<CompletionItem>) -> Self {
        let filtered: Vec<usize> = (0..items.len()).collect();
        Self {
            all_items: items,
            filtered,
            selected: 0,
            filter: String::new(),
        }
    }

    /// Update the filter and recompute matched items.
    pub fn set_filter(&mut self, filter: &str) {
        self.filter = filter.to_string();
        let lower = filter.to_lowercase();
        self.filtered = self
            .all_items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.value.to_lowercase().starts_with(&lower))
            .map(|(i, _)| i)
            .collect();
        // Clamp selection.
        if self.filtered.is_empty() {
            self.selected = 0;
        } else if self.selected >= self.filtered.len() {
            self.selected = self.filtered.len() - 1;
        }
    }

    /// Get the filtered items for display.
    pub fn visible_items(&self) -> Vec<&CompletionItem> {
        self.filtered.iter().map(|&i| &self.all_items[i]).collect()
    }

    /// Get the currently selected item, if any.
    pub fn selected_item(&self) -> Option<&CompletionItem> {
        self.filtered
            .get(self.selected)
            .map(|&i| &self.all_items[i])
    }

    /// Returns true if there are no matching items.
    pub fn is_empty(&self) -> bool {
        self.filtered.is_empty()
    }

    /// Number of rows this popup needs to display.
    pub fn popup_height(&self) -> u16 {
        (self.filtered.len().min(MAX_POPUP_ROWS)) as u16
    }

    /// Move selection up (wraps around).
    pub fn move_up(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        if self.selected == 0 {
            self.selected = self.filtered.len() - 1;
        } else {
            self.selected -= 1;
        }
    }

    /// Move selection down (wraps around).
    pub fn move_down(&mut self) {
        if self.filtered.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.filtered.len();
    }
}

impl ActivePopup {
    /// Returns the number of extra rows needed for the popup (0 if none).
    pub fn extra_height(&self) -> u16 {
        match self {
            ActivePopup::None => 0,
            ActivePopup::SlashCommand(state)
            | ActivePopup::AgentName(state)
            | ActivePopup::WhisperName(state)
            | ActivePopup::SessionPicker(state) => {
                if state.is_empty() {
                    0
                } else {
                    // Popup rows replace the 1-row status bar, so net extra = popup_height - 1.
                    state.popup_height().saturating_sub(1)
                }
            }
        }
    }

    /// Returns true if a popup is active and has items to show.
    pub fn is_active(&self) -> bool {
        match self {
            ActivePopup::None => false,
            ActivePopup::SlashCommand(s)
            | ActivePopup::AgentName(s)
            | ActivePopup::WhisperName(s)
            | ActivePopup::SessionPicker(s) => !s.is_empty(),
        }
    }

    /// Build the lines to render for the popup.
    pub fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        let state = match self {
            ActivePopup::None => return vec![],
            ActivePopup::SlashCommand(s)
            | ActivePopup::AgentName(s)
            | ActivePopup::WhisperName(s)
            | ActivePopup::SessionPicker(s) => s,
        };

        let items = state.visible_items();
        let visible_count = items.len().min(MAX_POPUP_ROWS);

        // Determine scroll window.
        let scroll_top = if state.selected < visible_count {
            0
        } else {
            state.selected + 1 - visible_count
        };

        let mut lines = Vec::with_capacity(visible_count);
        for (display_idx, &item) in items
            .iter()
            .enumerate()
            .skip(scroll_top)
            .take(visible_count)
        {
            let is_selected = display_idx == state.selected;
            let line = render_popup_line(item, is_selected, width);
            lines.push(line);
        }

        lines
    }
}

/// Render a single popup line.
fn render_popup_line(item: &CompletionItem, selected: bool, _width: u16) -> Line<'static> {
    let marker = if selected { "▸ " } else { "  " };
    let name_style = if selected {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::White)
    };
    let desc_style = if selected {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    Line::from(vec![
        Span::styled(marker.to_string(), name_style),
        Span::styled(format!("{:<12}", item.value), name_style),
        Span::styled(item.description.clone(), desc_style),
    ])
}
