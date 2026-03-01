//! Content inserted above the viewport (scrolls into terminal history).

use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

use crate::custom_terminal;

/// Insert lines above the viewport with a trailing blank line.
///
/// Used for command output, error messages, user messages, and echo replies.
pub fn insert_lines(
    terminal: &mut custom_terminal::Terminal,
    lines: Vec<Line<'static>>,
) -> anyhow::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    // Message lines + 1 blank line after.
    let height = (lines.len() + 1) as u16;
    terminal.insert_before(height, |buf| {
        let area = Rect::new(0, 0, buf.area.width, lines.len() as u16);
        let paragraph = Paragraph::new(lines);
        ratatui::widgets::Widget::render(paragraph, area, buf);
    })?;
    Ok(())
}
