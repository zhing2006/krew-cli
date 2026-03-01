//! Completion popup rendering.

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::custom_terminal;

/// Render completion popup lines in the given area.
pub fn render_popup(frame: &mut custom_terminal::Frame, lines: Vec<Line<'static>>, area: Rect) {
    let paragraph = Paragraph::new(lines);
    frame.render_widget(paragraph, area);
}
