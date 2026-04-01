//! Custom TextArea widget with atomic element support.
//!
//! This is a single-buffer text editor widget with:
//! - Byte-level cursor positioning
//! - Atomic "element" ranges that can't be partially edited (used for
//!   large paste placeholders like `[Pasted text (1234 chars)]`)
//! - Word wrapping via `textwrap`
//! - Emacs-like keybindings (Ctrl+B/F/P/N, kill ring, word ops)

use crossterm::event::KeyCode;
use crossterm::event::KeyEvent;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use std::cell::Ref;
use std::cell::RefCell;
use std::ops::Range;
use textwrap::Options;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const WORD_SEPARATORS: &str = "`~!@#$%^&*()-=+[{]}\\|;:'\",.<>/?";

fn is_word_separator(ch: char) -> bool {
    WORD_SEPARATORS.contains(ch)
}

/// Check if key modifiers represent AltGr (Windows sends ALT|CONTROL).
#[cfg(windows)]
fn is_altgr(mods: KeyModifiers) -> bool {
    mods.contains(KeyModifiers::ALT) && mods.contains(KeyModifiers::CONTROL)
}

#[cfg(not(windows))]
fn is_altgr(_mods: KeyModifiers) -> bool {
    false
}

// ── Word wrapping helpers ────────────────────────────────────────────

/// Returns byte-ranges into `text` for each wrapped line, including
/// trailing whitespace and a +1 sentinel byte.  Used by the cursor
/// position logic.
fn wrap_ranges<'a, O>(text: &str, width_or_options: O) -> Vec<Range<usize>>
where
    O: Into<Options<'a>>,
{
    let opts = width_or_options.into();
    let mut lines: Vec<Range<usize>> = Vec::new();
    let mut cursor = 0usize;
    for (line_index, line) in textwrap::wrap(text, &opts).iter().enumerate() {
        match line {
            std::borrow::Cow::Borrowed(slice) => {
                let start = unsafe { slice.as_ptr().offset_from(text.as_ptr()) as usize };
                let end = start + slice.len();
                let trailing_spaces = text[end..].chars().take_while(|c| *c == ' ').count();
                lines.push(start..end + trailing_spaces + 1);
                cursor = end + trailing_spaces;
            }
            std::borrow::Cow::Owned(slice) => {
                let synthetic_prefix = if line_index == 0 {
                    opts.initial_indent
                } else {
                    opts.subsequent_indent
                };
                let mapped = map_owned_wrapped_line_to_range(text, cursor, slice, synthetic_prefix);
                let trailing_spaces = text[mapped.end..].chars().take_while(|c| *c == ' ').count();
                lines.push(mapped.start..mapped.end + trailing_spaces + 1);
                cursor = mapped.end + trailing_spaces;
            }
        }
    }
    lines
}

/// Maps an owned (materialized) wrapped line back to a byte range in `text`.
///
/// `textwrap` returns `Cow::Owned` when it inserts a hyphenation penalty
/// character (typically `-`) that does not exist in the source.
fn map_owned_wrapped_line_to_range(
    text: &str,
    cursor: usize,
    wrapped: &str,
    synthetic_prefix: &str,
) -> Range<usize> {
    let wrapped = if synthetic_prefix.is_empty() {
        wrapped
    } else {
        wrapped.strip_prefix(synthetic_prefix).unwrap_or(wrapped)
    };

    let mut start = cursor;
    while start < text.len() && !wrapped.starts_with(' ') {
        let Some(ch) = text[start..].chars().next() else {
            break;
        };
        if ch != ' ' {
            break;
        }
        start += ch.len_utf8();
    }

    let mut src_idx = start;
    let mut wrapped_chars = wrapped.chars().peekable();
    while let Some(&wc) = wrapped_chars.peek() {
        if src_idx >= text.len() {
            break;
        }
        let Some(sc) = text[src_idx..].chars().next() else {
            break;
        };
        if wc == sc {
            src_idx += sc.len_utf8();
            wrapped_chars.next();
        } else {
            // Penalty char inserted by textwrap — skip it in the wrapped string.
            break;
        }
    }

    start..src_idx
}

// ── Internal element type ────────────────────────────────────────────

#[derive(Debug, Clone)]
struct TextElement {
    id: u64,
    range: Range<usize>,
    name: Option<String>,
}

// ── TextArea ─────────────────────────────────────────────────────────

#[derive(Debug)]
pub struct TextArea {
    text: String,
    cursor_pos: usize,
    wrap_cache: RefCell<WrapCache>,
    preferred_col: Option<usize>,
    elements: Vec<TextElement>,
    next_element_id: u64,
    kill_buffer: String,
}

#[derive(Debug, Clone)]
struct WrapCache {
    width: u16,
    lines: Vec<Range<usize>>,
}

impl TextArea {
    pub fn new() -> Self {
        Self {
            text: String::new(),
            cursor_pos: 0,
            wrap_cache: RefCell::new(WrapCache {
                width: 0,
                lines: Vec::new(),
            }),
            preferred_col: None,
            elements: Vec::new(),
            next_element_id: 1,
            kill_buffer: String::new(),
        }
    }

    // ── Compatibility helpers ────────────────────────────────────────

    /// Return text split by newlines.
    pub fn lines(&self) -> Vec<&str> {
        if self.text.is_empty() {
            vec![""]
        } else {
            self.text.split('\n').collect()
        }
    }

    /// Number of logical lines (newline-separated).
    pub fn line_count(&self) -> usize {
        if self.text.is_empty() {
            1
        } else {
            self.text.matches('\n').count() + 1
        }
    }

    /// Convert byte cursor position to (row, col) in characters.
    pub fn cursor_row_col(&self) -> (usize, usize) {
        let before = &self.text[..self.cursor_pos];
        let row = before.matches('\n').count();
        let last_nl = before.rfind('\n').map(|i| i + 1).unwrap_or(0);
        let col = before[last_nl..].chars().count();
        (row, col)
    }

    /// Insert a newline at the cursor position.
    pub fn insert_newline(&mut self) {
        self.insert_str("\n");
    }

    // ── Core text operations ─────────────────────────────────────────

    /// Replace the textarea text and clear any existing text elements.
    pub fn set_text_clearing_elements(&mut self, text: &str) {
        self.text = text.to_string();
        self.cursor_pos = self.cursor_pos.clamp(0, self.text.len());
        self.elements.clear();
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.wrap_cache.borrow_mut().width = 0;
        self.preferred_col = None;
        self.kill_buffer.clear();
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn insert_str(&mut self, text: &str) {
        self.insert_str_at(self.cursor_pos, text);
    }

    pub fn insert_str_at(&mut self, pos: usize, text: &str) {
        let pos = self.clamp_pos_for_insertion(pos);
        self.text.insert_str(pos, text);
        self.wrap_cache.borrow_mut().width = 0;
        if pos <= self.cursor_pos {
            self.cursor_pos += text.len();
        }
        self.shift_elements(pos, 0, text.len());
        self.preferred_col = None;
    }

    pub fn replace_range(&mut self, range: std::ops::Range<usize>, text: &str) {
        let range = self.expand_range_to_element_boundaries(range);
        self.replace_range_raw(range, text);
    }

    fn replace_range_raw(&mut self, range: Range<usize>, text: &str) {
        assert!(range.start <= range.end);
        let start = range.start.clamp(0, self.text.len());
        let end = range.end.clamp(0, self.text.len());
        let removed_len = end - start;
        let inserted_len = text.len();
        if removed_len == 0 && inserted_len == 0 {
            return;
        }
        let diff = inserted_len as isize - removed_len as isize;

        self.text.replace_range(range, text);
        self.wrap_cache.borrow_mut().width = 0;
        self.preferred_col = None;
        self.update_elements_after_replace(start, end, inserted_len);

        // Update the cursor position to account for the edit.
        self.cursor_pos = if self.cursor_pos < start {
            self.cursor_pos
        } else if self.cursor_pos <= end {
            start + inserted_len
        } else {
            ((self.cursor_pos as isize) + diff) as usize
        }
        .min(self.text.len());

        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    pub fn cursor(&self) -> usize {
        self.cursor_pos
    }

    pub fn set_cursor(&mut self, pos: usize) {
        self.cursor_pos = pos.clamp(0, self.text.len());
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    pub fn desired_height(&self, width: u16) -> u16 {
        self.wrapped_lines(width).len() as u16
    }

    /// Compute the on-screen cursor position relative to the given area,
    /// accounting for scroll offset.
    pub fn cursor_pos_in(&self, area: Rect) -> Option<(u16, u16)> {
        let lines = self.wrapped_lines(area.width);
        let scroll = self.effective_scroll(area.height, &lines, 0);
        let i = Self::wrapped_line_index_by_start(&lines, self.cursor_pos)?;
        let screen_row = (i as u16).checked_sub(scroll)?;
        if screen_row >= area.height {
            return None;
        }
        let ls = &lines[i];
        let col = self.text[ls.start..self.cursor_pos].width() as u16;
        Some((area.x + col, area.y + screen_row))
    }

    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    // ── Keyboard input ───────────────────────────────────────────────

    pub fn input(&mut self, event: KeyEvent) {
        if !matches!(event.kind, KeyEventKind::Press | KeyEventKind::Repeat) {
            return;
        }
        match event {
            // C0 control character fallbacks (some terminals send these without CONTROL modifier).
            KeyEvent {
                code: KeyCode::Char('\u{0002}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Char('\u{0006}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char('\u{0010}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Char('\u{000e}'),
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_down();
            }
            // Plain character input (no ALT — many terminals map Option to ALT for word nav).
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers: KeyModifiers::NONE | KeyModifiers::SHIFT,
                ..
            } => self.insert_str(&c.to_string()),
            // Enter / Ctrl+J / Ctrl+M → newline
            KeyEvent {
                code: KeyCode::Char('j' | 'm'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
            | KeyEvent {
                code: KeyCode::Enter,
                ..
            } => self.insert_str("\n"),
            // Ctrl+Alt+H → delete backward word
            KeyEvent {
                code: KeyCode::Char('h'),
                modifiers,
                ..
            } if modifiers == (KeyModifiers::CONTROL | KeyModifiers::ALT) => {
                self.delete_backward_word();
            }
            // Windows AltGr (ALT|CONTROL) → treat as plain char
            KeyEvent {
                code: KeyCode::Char(c),
                modifiers,
                ..
            } if is_altgr(modifiers) => self.insert_str(&c.to_string()),
            // Alt+Backspace → delete backward word
            KeyEvent {
                code: KeyCode::Backspace,
                modifiers: KeyModifiers::ALT,
                ..
            } => self.delete_backward_word(),
            // Backspace / Ctrl+H → delete backward
            KeyEvent {
                code: KeyCode::Backspace,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.delete_backward(1),
            // Alt+Delete / Alt+D → delete forward word
            KeyEvent {
                code: KeyCode::Delete,
                modifiers: KeyModifiers::ALT,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::ALT,
                ..
            } => self.delete_forward_word(),
            // Delete / Ctrl+D → delete forward
            KeyEvent {
                code: KeyCode::Delete,
                ..
            }
            | KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.delete_forward(1),
            // Ctrl+W → delete backward word
            KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.delete_backward_word(),
            // Alt+B → beginning of previous word
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::ALT,
                ..
            } => self.set_cursor(self.beginning_of_previous_word()),
            // Alt+F → end of next word
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::ALT,
                ..
            } => self.set_cursor(self.end_of_next_word()),
            // Ctrl+U → kill to beginning of line
            KeyEvent {
                code: KeyCode::Char('u'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.kill_to_beginning_of_line(),
            // Ctrl+K → kill to end of line
            KeyEvent {
                code: KeyCode::Char('k'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.kill_to_end_of_line(),
            // Ctrl+Y → yank
            KeyEvent {
                code: KeyCode::Char('y'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => self.yank(),
            // Arrow keys
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::NONE,
                ..
            } => {
                self.move_cursor_right();
            }
            // Ctrl+B/F/P/N → cursor movement
            KeyEvent {
                code: KeyCode::Char('b'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_left();
            }
            KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_right();
            }
            KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_up();
            }
            KeyEvent {
                code: KeyCode::Char('n'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_down();
            }
            // Alt+Left/Right or Ctrl+Left/Right → word-wise movement
            KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::ALT,
                ..
            }
            | KeyEvent {
                code: KeyCode::Left,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.set_cursor(self.beginning_of_previous_word());
            }
            KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::ALT,
                ..
            }
            | KeyEvent {
                code: KeyCode::Right,
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.set_cursor(self.end_of_next_word());
            }
            // Up/Down
            KeyEvent {
                code: KeyCode::Up, ..
            } => self.move_cursor_up(),
            KeyEvent {
                code: KeyCode::Down,
                ..
            } => self.move_cursor_down(),
            // Home / Ctrl+A → beginning of line
            KeyEvent {
                code: KeyCode::Home,
                ..
            } => {
                self.move_cursor_to_beginning_of_line(false);
            }
            KeyEvent {
                code: KeyCode::Char('a'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_to_beginning_of_line(true);
            }
            // End / Ctrl+E → end of line
            KeyEvent {
                code: KeyCode::End, ..
            } => {
                self.move_cursor_to_end_of_line(false);
            }
            KeyEvent {
                code: KeyCode::Char('e'),
                modifiers: KeyModifiers::CONTROL,
                ..
            } => {
                self.move_cursor_to_end_of_line(true);
            }
            _ => {}
        }
    }

    // ── Deletion operations ──────────────────────────────────────────

    pub fn delete_backward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos == 0 {
            return;
        }
        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.prev_atomic_boundary(target);
            if target == 0 {
                break;
            }
        }
        self.replace_range(target..self.cursor_pos, "");
    }

    pub fn delete_forward(&mut self, n: usize) {
        if n == 0 || self.cursor_pos >= self.text.len() {
            return;
        }
        let mut target = self.cursor_pos;
        for _ in 0..n {
            target = self.next_atomic_boundary(target);
            if target >= self.text.len() {
                break;
            }
        }
        self.replace_range(self.cursor_pos..target, "");
    }

    pub fn delete_backward_word(&mut self) {
        let start = self.beginning_of_previous_word();
        self.kill_range(start..self.cursor_pos);
    }

    pub fn delete_forward_word(&mut self) {
        let end = self.end_of_next_word();
        if end > self.cursor_pos {
            self.kill_range(self.cursor_pos..end);
        }
    }

    pub fn kill_to_end_of_line(&mut self) {
        let eol = self.end_of_current_line();
        let range = if self.cursor_pos == eol {
            if eol < self.text.len() {
                Some(self.cursor_pos..eol + 1)
            } else {
                None
            }
        } else {
            Some(self.cursor_pos..eol)
        };
        if let Some(range) = range {
            self.kill_range(range);
        }
    }

    pub fn kill_to_beginning_of_line(&mut self) {
        let bol = self.beginning_of_current_line();
        let range = if self.cursor_pos == bol {
            if bol > 0 { Some(bol - 1..bol) } else { None }
        } else {
            Some(bol..self.cursor_pos)
        };
        if let Some(range) = range {
            self.kill_range(range);
        }
    }

    pub fn yank(&mut self) {
        if self.kill_buffer.is_empty() {
            return;
        }
        let text = self.kill_buffer.clone();
        self.insert_str(&text);
    }

    fn kill_range(&mut self, range: Range<usize>) {
        let range = self.expand_range_to_element_boundaries(range);
        if range.start >= range.end {
            return;
        }
        let removed = self.text[range.clone()].to_string();
        if removed.is_empty() {
            return;
        }
        self.kill_buffer = removed;
        self.replace_range_raw(range, "");
    }

    // ── Cursor movement ──────────────────────────────────────────────

    pub fn move_cursor_left(&mut self) {
        self.cursor_pos = self.prev_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    pub fn move_cursor_right(&mut self) {
        self.cursor_pos = self.next_atomic_boundary(self.cursor_pos);
        self.preferred_col = None;
    }

    pub fn move_cursor_up(&mut self) {
        if let Some((target_col, maybe_line)) = {
            let cache_ref = self.wrap_cache.borrow();
            let lines = &cache_ref.lines;
            if lines.is_empty() {
                None
            } else if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                let cur_range = &lines[idx];
                let target_col = self
                    .preferred_col
                    .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());
                if idx > 0 {
                    let prev = &lines[idx - 1];
                    let line_start = prev.start;
                    let line_end = prev.end.saturating_sub(1);
                    Some((target_col, Some((line_start, line_end))))
                } else {
                    Some((target_col, None))
                }
            } else {
                None
            }
        } {
            match maybe_line {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    self.cursor_pos = 0;
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation.
        if let Some(prev_nl) = self.text[..self.cursor_pos].rfind('\n') {
            let target_col = match self.preferred_col {
                Some(c) => c,
                None => {
                    let c = self.current_display_col();
                    self.preferred_col = Some(c);
                    c
                }
            };
            let prev_line_start = self.text[..prev_nl].rfind('\n').map(|i| i + 1).unwrap_or(0);
            let prev_line_end = prev_nl;
            self.move_to_display_col_on_line(prev_line_start, prev_line_end, target_col);
        } else {
            self.cursor_pos = 0;
            self.preferred_col = None;
        }
    }

    pub fn move_cursor_down(&mut self) {
        if let Some((target_col, move_to_last)) = {
            let cache_ref = self.wrap_cache.borrow();
            let lines = &cache_ref.lines;
            if lines.is_empty() {
                None
            } else if let Some(idx) = Self::wrapped_line_index_by_start(lines, self.cursor_pos) {
                let cur_range = &lines[idx];
                let target_col = self
                    .preferred_col
                    .unwrap_or_else(|| self.text[cur_range.start..self.cursor_pos].width());
                if idx + 1 < lines.len() {
                    let next = &lines[idx + 1];
                    let line_start = next.start;
                    let line_end = next.end.saturating_sub(1);
                    Some((target_col, Some((line_start, line_end))))
                } else {
                    Some((target_col, None))
                }
            } else {
                None
            }
        } {
            match move_to_last {
                Some((line_start, line_end)) => {
                    if self.preferred_col.is_none() {
                        self.preferred_col = Some(target_col);
                    }
                    self.move_to_display_col_on_line(line_start, line_end, target_col);
                    return;
                }
                None => {
                    self.cursor_pos = self.text.len();
                    self.preferred_col = None;
                    return;
                }
            }
        }

        // Fallback to logical line navigation.
        let target_col = match self.preferred_col {
            Some(c) => c,
            None => {
                let c = self.current_display_col();
                self.preferred_col = Some(c);
                c
            }
        };
        if let Some(next_nl) = self.text[self.cursor_pos..]
            .find('\n')
            .map(|i| i + self.cursor_pos)
        {
            let next_line_start = next_nl + 1;
            let next_line_end = self.text[next_line_start..]
                .find('\n')
                .map(|i| i + next_line_start)
                .unwrap_or(self.text.len());
            self.move_to_display_col_on_line(next_line_start, next_line_end, target_col);
        } else {
            self.cursor_pos = self.text.len();
            self.preferred_col = None;
        }
    }

    pub fn move_cursor_to_beginning_of_line(&mut self, move_up_at_bol: bool) {
        let bol = self.beginning_of_current_line();
        if move_up_at_bol && self.cursor_pos == bol {
            self.set_cursor(self.beginning_of_line(self.cursor_pos.saturating_sub(1)));
        } else {
            self.set_cursor(bol);
        }
        self.preferred_col = None;
    }

    pub fn move_cursor_to_end_of_line(&mut self, move_down_at_eol: bool) {
        let eol = self.end_of_current_line();
        if move_down_at_eol && self.cursor_pos == eol {
            let next_pos = (self.cursor_pos.saturating_add(1)).min(self.text.len());
            self.set_cursor(self.end_of_line(next_pos));
        } else {
            self.set_cursor(eol);
        }
    }

    // ── Text elements support ────────────────────────────────────────

    /// Get the text content of all tracked elements.
    pub fn element_payloads(&self) -> Vec<String> {
        self.elements
            .iter()
            .filter_map(|e| self.text.get(e.range.clone()).map(str::to_string))
            .collect()
    }

    /// Return (id, range) pairs for all tracked elements.
    pub fn elements_snapshot(&self) -> Vec<(u64, Range<usize>)> {
        self.elements
            .iter()
            .map(|e| (e.id, e.range.clone()))
            .collect()
    }

    /// Insert text and mark it as an atomic element. Returns the element ID.
    pub fn insert_element(&mut self, text: &str) -> u64 {
        let start = self.clamp_pos_for_insertion(self.cursor_pos);
        self.insert_str_at(start, text);
        let end = start + text.len();
        let id = self.add_element(start..end);
        self.set_cursor(end);
        id
    }

    /// Renames a single text element in-place, keeping it atomic.
    pub fn replace_element_payload(&mut self, old: &str, new: &str) -> bool {
        let Some(idx) = self
            .elements
            .iter()
            .position(|e| self.text.get(e.range.clone()) == Some(old))
        else {
            return false;
        };

        let range = self.elements[idx].range.clone();
        let start = range.start;
        let end = range.end;
        if start > end || end > self.text.len() {
            return false;
        }

        let removed_len = end - start;
        let inserted_len = new.len();
        let diff = inserted_len as isize - removed_len as isize;

        self.text.replace_range(range, new);
        self.wrap_cache.borrow_mut().width = 0;
        self.preferred_col = None;

        self.elements[idx].range = start..(start + inserted_len);

        if diff != 0 {
            for (j, e) in self.elements.iter_mut().enumerate() {
                if j == idx {
                    continue;
                }
                if e.range.end <= start {
                    continue;
                }
                if e.range.start >= end {
                    e.range.start = ((e.range.start as isize) + diff) as usize;
                    e.range.end = ((e.range.end as isize) + diff) as usize;
                    continue;
                }
                e.range.start = start.min(e.range.start);
                e.range.end = (start + inserted_len).max(e.range.end.saturating_add_signed(diff));
            }
        }

        self.cursor_pos = if self.cursor_pos < start {
            self.cursor_pos
        } else if self.cursor_pos <= end {
            start + inserted_len
        } else {
            ((self.cursor_pos as isize) + diff) as usize
        };
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
        self.elements.sort_by_key(|e| e.range.start);

        true
    }

    pub fn replace_element_by_id(&mut self, id: &str, text: &str) -> bool {
        if let Some(idx) = self
            .elements
            .iter()
            .position(|e| e.name.as_deref() == Some(id))
        {
            let range = self.elements[idx].range.clone();
            self.replace_range_raw(range, text);
            self.elements.retain(|e| e.name.as_deref() != Some(id));
            true
        } else {
            false
        }
    }

    /// Mark an existing text range as an atomic element without changing the text.
    pub fn add_element_range(&mut self, range: Range<usize>) -> Option<u64> {
        let start = self.clamp_pos_to_char_boundary(range.start.min(self.text.len()));
        let end = self.clamp_pos_to_char_boundary(range.end.min(self.text.len()));
        if start >= end {
            return None;
        }
        if self
            .elements
            .iter()
            .any(|e| e.range.start == start && e.range.end == end)
        {
            return None;
        }
        if self
            .elements
            .iter()
            .any(|e| start < e.range.end && end > e.range.start)
        {
            return None;
        }
        let id = self.add_element(start..end);
        Some(id)
    }

    pub fn remove_element_range(&mut self, range: Range<usize>) -> bool {
        let start = self.clamp_pos_to_char_boundary(range.start.min(self.text.len()));
        let end = self.clamp_pos_to_char_boundary(range.end.min(self.text.len()));
        if start >= end {
            return false;
        }
        let len_before = self.elements.len();
        self.elements
            .retain(|elem| elem.range.start != start || elem.range.end != end);
        len_before != self.elements.len()
    }

    // ── Private helpers ──────────────────────────────────────────────

    fn current_display_col(&self) -> usize {
        let bol = self.beginning_of_current_line();
        self.text[bol..self.cursor_pos].width()
    }

    fn wrapped_line_index_by_start(lines: &[Range<usize>], pos: usize) -> Option<usize> {
        let idx = lines.partition_point(|r| r.start <= pos);
        if idx == 0 { None } else { Some(idx - 1) }
    }

    fn move_to_display_col_on_line(
        &mut self,
        line_start: usize,
        line_end: usize,
        target_col: usize,
    ) {
        let mut width_so_far = 0usize;
        for (i, g) in self.text[line_start..line_end].grapheme_indices(true) {
            width_so_far += g.width();
            if width_so_far > target_col {
                self.cursor_pos = line_start + i;
                self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
                return;
            }
        }
        self.cursor_pos = line_end;
        self.cursor_pos = self.clamp_pos_to_nearest_boundary(self.cursor_pos);
    }

    fn beginning_of_line(&self, pos: usize) -> usize {
        self.text[..pos].rfind('\n').map(|i| i + 1).unwrap_or(0)
    }

    fn beginning_of_current_line(&self) -> usize {
        self.beginning_of_line(self.cursor_pos)
    }

    fn end_of_line(&self, pos: usize) -> usize {
        self.text[pos..]
            .find('\n')
            .map(|i| i + pos)
            .unwrap_or(self.text.len())
    }

    fn end_of_current_line(&self) -> usize {
        self.end_of_line(self.cursor_pos)
    }

    fn beginning_of_previous_word(&self) -> usize {
        let prefix = &self.text[..self.cursor_pos];
        let Some((first_non_ws_idx, ch)) = prefix
            .char_indices()
            .rev()
            .find(|&(_, ch)| !ch.is_whitespace())
        else {
            return 0;
        };
        let is_separator = is_word_separator(ch);
        let mut start = first_non_ws_idx;
        for (idx, ch) in prefix[..first_non_ws_idx].char_indices().rev() {
            if ch.is_whitespace() || is_word_separator(ch) != is_separator {
                start = idx + ch.len_utf8();
                break;
            }
            start = idx;
        }
        self.adjust_pos_out_of_elements(start, true)
    }

    fn end_of_next_word(&self) -> usize {
        let Some(first_non_ws) = self.text[self.cursor_pos..].find(|c: char| !c.is_whitespace())
        else {
            return self.text.len();
        };
        let word_start = self.cursor_pos + first_non_ws;
        let mut iter = self.text[word_start..].char_indices();
        let Some((_, first_ch)) = iter.next() else {
            return word_start;
        };
        let is_separator = is_word_separator(first_ch);
        let mut end = self.text.len();
        for (idx, ch) in iter {
            if ch.is_whitespace() || is_word_separator(ch) != is_separator {
                end = word_start + idx;
                break;
            }
        }
        self.adjust_pos_out_of_elements(end, false)
    }

    fn adjust_pos_out_of_elements(&self, pos: usize, prefer_start: bool) -> usize {
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            if prefer_start {
                e.range.start
            } else {
                e.range.end
            }
        } else {
            pos
        }
    }

    fn add_element_with_id(&mut self, range: Range<usize>, name: Option<String>) -> u64 {
        let id = self.next_element_id();
        let elem = TextElement { id, range, name };
        self.elements.push(elem);
        self.elements.sort_by_key(|e| e.range.start);
        id
    }

    fn add_element(&mut self, range: Range<usize>) -> u64 {
        self.add_element_with_id(range, None)
    }

    fn next_element_id(&mut self) -> u64 {
        let id = self.next_element_id;
        self.next_element_id = self.next_element_id.saturating_add(1);
        id
    }

    fn find_element_containing(&self, pos: usize) -> Option<usize> {
        self.elements
            .iter()
            .position(|e| pos > e.range.start && pos < e.range.end)
    }

    fn clamp_pos_to_char_boundary(&self, pos: usize) -> usize {
        let pos = pos.min(self.text.len());
        if self.text.is_char_boundary(pos) {
            return pos;
        }
        let mut prev = pos;
        while prev > 0 && !self.text.is_char_boundary(prev) {
            prev -= 1;
        }
        let mut next = pos;
        while next < self.text.len() && !self.text.is_char_boundary(next) {
            next += 1;
        }
        if pos.saturating_sub(prev) <= next.saturating_sub(pos) {
            prev
        } else {
            next
        }
    }

    fn clamp_pos_to_nearest_boundary(&self, pos: usize) -> usize {
        let pos = self.clamp_pos_to_char_boundary(pos);
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);
            if dist_start <= dist_end {
                self.clamp_pos_to_char_boundary(e.range.start)
            } else {
                self.clamp_pos_to_char_boundary(e.range.end)
            }
        } else {
            pos
        }
    }

    fn clamp_pos_for_insertion(&self, pos: usize) -> usize {
        let pos = self.clamp_pos_to_char_boundary(pos);
        if let Some(idx) = self.find_element_containing(pos) {
            let e = &self.elements[idx];
            let dist_start = pos.saturating_sub(e.range.start);
            let dist_end = e.range.end.saturating_sub(pos);
            if dist_start <= dist_end {
                self.clamp_pos_to_char_boundary(e.range.start)
            } else {
                self.clamp_pos_to_char_boundary(e.range.end)
            }
        } else {
            pos
        }
    }

    fn expand_range_to_element_boundaries(&self, mut range: Range<usize>) -> Range<usize> {
        loop {
            let mut changed = false;
            for e in &self.elements {
                if e.range.start < range.end && e.range.end > range.start {
                    let new_start = range.start.min(e.range.start);
                    let new_end = range.end.max(e.range.end);
                    if new_start != range.start || new_end != range.end {
                        range.start = new_start;
                        range.end = new_end;
                        changed = true;
                    }
                }
            }
            if !changed {
                break;
            }
        }
        range
    }

    fn shift_elements(&mut self, at: usize, removed: usize, inserted: usize) {
        let end = at + removed;
        let diff = inserted as isize - removed as isize;
        self.elements
            .retain(|e| !(e.range.start >= at && e.range.end <= end));
        for e in &mut self.elements {
            if e.range.end <= at {
                // before edit
            } else if e.range.start >= end {
                // after edit
                e.range.start = ((e.range.start as isize) + diff) as usize;
                e.range.end = ((e.range.end as isize) + diff) as usize;
            } else {
                // overlap — degrade gracefully
                let new_start = at.min(e.range.start);
                let new_end = at + inserted.max(e.range.end.saturating_sub(end));
                e.range.start = new_start;
                e.range.end = new_end;
            }
        }
    }

    fn update_elements_after_replace(&mut self, start: usize, end: usize, inserted_len: usize) {
        self.shift_elements(start, end.saturating_sub(start), inserted_len);
    }

    fn prev_atomic_boundary(&self, pos: usize) -> usize {
        if pos == 0 {
            return 0;
        }
        if let Some(idx) = self
            .elements
            .iter()
            .position(|e| pos > e.range.start && pos <= e.range.end)
        {
            return self.elements[idx].range.start;
        }
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.prev_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.start
                } else {
                    b
                }
            }
            Ok(None) => 0,
            Err(_) => pos.saturating_sub(1),
        }
    }

    fn next_atomic_boundary(&self, pos: usize) -> usize {
        if pos >= self.text.len() {
            return self.text.len();
        }
        if let Some(idx) = self
            .elements
            .iter()
            .position(|e| pos >= e.range.start && pos < e.range.end)
        {
            return self.elements[idx].range.end;
        }
        let mut gc = unicode_segmentation::GraphemeCursor::new(pos, self.text.len(), false);
        match gc.next_boundary(&self.text, 0) {
            Ok(Some(b)) => {
                if let Some(idx) = self.find_element_containing(b) {
                    self.elements[idx].range.end
                } else {
                    b
                }
            }
            Ok(None) => self.text.len(),
            Err(_) => pos.saturating_add(1),
        }
    }

    // ── Wrapping ─────────────────────────────────────────────────────

    fn wrapped_lines(&self, width: u16) -> Ref<'_, Vec<Range<usize>>> {
        {
            let mut cache = self.wrap_cache.borrow_mut();
            if cache.width != width {
                let lines = wrap_ranges(
                    &self.text,
                    Options::new(width as usize).wrap_algorithm(textwrap::WrapAlgorithm::FirstFit),
                );
                cache.width = width;
                cache.lines = lines;
            }
        }
        let cache = self.wrap_cache.borrow();
        Ref::map(cache, |c| &c.lines)
    }

    fn effective_scroll(
        &self,
        area_height: u16,
        lines: &[Range<usize>],
        current_scroll: u16,
    ) -> u16 {
        let total_lines = lines.len() as u16;
        if area_height >= total_lines {
            return 0;
        }
        let cursor_line_idx =
            Self::wrapped_line_index_by_start(lines, self.cursor_pos).unwrap_or(0) as u16;
        let max_scroll = total_lines.saturating_sub(area_height);
        let mut scroll = current_scroll.min(max_scroll);
        if cursor_line_idx < scroll {
            scroll = cursor_line_idx;
        } else if cursor_line_idx >= scroll + area_height {
            scroll = cursor_line_idx + 1 - area_height;
        }
        scroll
    }

    // ── Rendering ────────────────────────────────────────────────────

    fn render_lines(
        &self,
        area: Rect,
        buf: &mut Buffer,
        lines: &[Range<usize>],
        range: std::ops::Range<usize>,
    ) {
        for (row, idx) in range.enumerate() {
            let r = &lines[idx];
            let y = area.y + row as u16;
            let line_range = r.start..r.end - 1;
            // Draw base line with default style.
            buf.set_string(area.x, y, &self.text[line_range.clone()], Style::default());

            // Overlay styled segments for elements that intersect this line.
            for elem in &self.elements {
                let overlap_start = elem.range.start.max(line_range.start);
                let overlap_end = elem.range.end.min(line_range.end);
                if overlap_start >= overlap_end {
                    continue;
                }
                let styled = &self.text[overlap_start..overlap_end];
                let x_off = self.text[line_range.start..overlap_start].width() as u16;
                let style = Style::default().fg(Color::Cyan);
                buf.set_string(area.x + x_off, y, styled, style);
            }
        }
    }
}

// ── Widget implementation ─────────────────────────────────────────────

impl Widget for &TextArea {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let lines = self.wrapped_lines(area.width);
        let scroll = self.effective_scroll(area.height, &lines, 0);
        let start = scroll as usize;
        let end = (scroll + area.height).min(lines.len() as u16) as usize;
        self.render_lines(area, buf, &lines, start..end);
    }
}
