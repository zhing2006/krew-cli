//! Content inserted above the viewport (scrolls into terminal history).

use ratatui::text::Line;

use crate::custom_terminal;

/// Insert lines above the viewport with a trailing blank line.
///
/// Uses direct ANSI output with real `\r\n` line breaks so the
/// terminal emulator preserves newlines when copying text.
pub fn insert_lines(
    terminal: &mut custom_terminal::Terminal,
    lines: Vec<Line<'static>>,
) -> anyhow::Result<()> {
    if lines.is_empty() {
        return Ok(());
    }
    // Add a trailing blank line for visual spacing.
    let mut all_lines = lines;
    all_lines.push(Line::raw(""));
    terminal.insert_lines_above(all_lines)?;
    Ok(())
}
