//! Startup header box rendering.

use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::custom_terminal;

use super::parse_color;

/// Shorten a path to fit within `max_width` columns by collapsing the middle.
fn shorten_path(path: &str, max_width: usize) -> String {
    if path.width() <= max_width {
        return path.to_string();
    }
    if max_width <= 3 {
        return ".".repeat(max_width);
    }

    let available_width = max_width - 3;
    let head_width = available_width / 2;
    let tail_width = available_width - head_width;

    let mut head = String::new();
    let mut used_width = 0;
    for grapheme in path.graphemes(true) {
        let width = grapheme.width();
        if used_width + width > head_width {
            break;
        }
        head.push_str(grapheme);
        used_width += width;
    }

    let mut tail_graphemes = Vec::new();
    used_width = 0;
    for grapheme in path.graphemes(true).rev() {
        let width = grapheme.width();
        if used_width + width > tail_width {
            break;
        }
        tail_graphemes.push(grapheme);
        used_width += width;
    }
    tail_graphemes.reverse();
    let tail = tail_graphemes.concat();

    format!("{head}...{tail}")
}

/// Greedily pack chunk widths into rows within `max_w` columns.
///
/// Row 0 starts at `first_prefix` columns, continuation rows at `cont_prefix`;
/// chunks on the same row are separated by `sep` columns. Every row holds at
/// least one chunk, so an oversized chunk overflows instead of looping.
fn pack_rows(
    chunk_widths: &[usize],
    first_prefix: usize,
    cont_prefix: usize,
    sep: usize,
    max_w: usize,
) -> Vec<Vec<usize>> {
    let mut rows: Vec<Vec<usize>> = Vec::new();
    let mut row: Vec<usize> = Vec::new();
    let mut used = first_prefix;
    for (i, &width) in chunk_widths.iter().enumerate() {
        if !row.is_empty() && used + sep + width > max_w {
            rows.push(std::mem::take(&mut row));
            used = cont_prefix;
        } else if !row.is_empty() {
            used += sep;
        }
        row.push(i);
        used += width;
    }
    if !row.is_empty() {
        rows.push(row);
    }
    rows
}

/// Insert the header box above the viewport.
pub fn insert_header(terminal: &mut custom_terminal::Terminal, app: &App) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let session_id = &app.session_id;
    let dir_full = app.cwd.display().to_string();
    let screen_w = terminal.size().map(|s| s.width).unwrap_or(120);

    // Build one chunk per agent so the Agents line can wrap across rows.
    let agents_label = " Agents: ";
    let separator = " | ";
    let mut chunks: Vec<(Vec<Span<'static>>, usize)> = Vec::new();
    for name in &app.config.settings.reply_order {
        if let Some(agent) = app.config.agents.iter().find(|a| &a.name == name) {
            let color = parse_color(&agent.color);
            let name_part = format!("[{}]", agent.name);
            let display_part = format!(" {}", agent.display_name);
            let width = name_part.width() + display_part.width();
            chunks.push((
                vec![
                    Span::styled(
                        name_part,
                        Style::default().fg(color).add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(display_part),
                ],
                width,
            ));
        }
    }

    let label_w = agents_label.width();
    let max_inner_w = (screen_w.saturating_sub(2) as usize).max(label_w + 1);
    let chunk_widths: Vec<usize> = chunks.iter().map(|(_, w)| *w).collect();
    let rows = pack_rows(
        &chunk_widths,
        label_w,
        label_w,
        separator.width(),
        max_inner_w,
    );

    // Continuation rows are indented to align under the label.
    let mut agents_lines: Vec<Line<'static>> = Vec::new();
    let mut agents_text_w = label_w + 1;
    for (row_idx, row) in rows.iter().enumerate() {
        let prefix = if row_idx == 0 {
            agents_label.to_string()
        } else {
            " ".repeat(label_w)
        };
        let mut spans = vec![Span::styled(prefix, Style::default().fg(Color::DarkGray))];
        let mut line_w = label_w;
        for (j, &chunk_idx) in row.iter().enumerate() {
            if j > 0 {
                spans.push(Span::styled(
                    separator.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
                line_w += separator.width();
            }
            let (chunk_spans, width) = &chunks[chunk_idx];
            spans.extend(chunk_spans.iter().cloned());
            line_w += width;
        }
        agents_text_w = agents_text_w.max(line_w + 1);
        agents_lines.push(Line::from(spans));
    }
    if agents_lines.is_empty() {
        agents_lines.push(Line::from(Span::styled(
            agents_label.to_string(),
            Style::default().fg(Color::DarkGray),
        )));
    }

    // Measure content widths to determine box width dynamically.
    // Line 1: " >_ Krew CLI (vX.Y.Z) — session_id"
    let line1_w = format!(" >_ Krew CLI (v{version}) — {session_id}").width();
    // Line 3: " Directory: <path>  Type /help for commands "
    let dir_label = " Directory: ";
    let right_part = "Type /help for commands ";
    let line3_min_w = dir_label.width() + dir_full.width() + 2 + right_part.width();

    // Inner width = max of all lines; box width = inner + 2 borders.
    let content_w = line1_w.max(agents_text_w).max(line3_min_w);
    // Clamp: at least 40, at most terminal width.
    let box_width = ((content_w + 2) as u16).clamp(40, screen_w);

    // Header box: content lines + 2 border lines, plus 1 blank line after.
    let box_height = (2 + agents_lines.len() + 2) as u16;
    let header_height = box_height + 1;

    terminal.insert_widget_above(header_height, |buf| {
        let inner_w = box_width.saturating_sub(2) as usize;

        // Build the third line with path truncation based on actual available width.
        let max_path = inner_w
            .saturating_sub(dir_label.len())
            .saturating_sub(right_part.len())
            .saturating_sub(1);
        let dir = shorten_path(&dir_full, max_path);
        let left_len = dir_label.width() + dir.width();
        let gap = inner_w.saturating_sub(left_len + right_part.width());

        let mut lines = vec![Line::from(vec![
            Span::styled(
                " >_ ",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("Krew CLI ", Style::default().add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("(v{version})"),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(
                format!(" \u{2014} {session_id}"),
                Style::default().fg(Color::DarkGray),
            ),
        ])];
        lines.extend(agents_lines.clone());
        lines.push(Line::from(vec![
            Span::styled(dir_label.to_string(), Style::default().fg(Color::DarkGray)),
            Span::raw(dir),
            Span::raw(" ".repeat(gap)),
            Span::styled("Type ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::styled(" for commands ", Style::default().fg(Color::DarkGray)),
        ]));

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let area = Rect::new(0, 0, box_width, box_height);
        let paragraph = Paragraph::new(lines).block(block);
        ratatui::widgets::Widget::render(paragraph, area, buf);
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{pack_rows, shorten_path};
    use unicode_width::UnicodeWidthStr;

    #[test]
    fn pack_rows_fits_all_chunks_on_one_row() {
        // 9 + 10 + 3 + 10 = 32 <= 40
        let rows = pack_rows(&[10, 10], 9, 9, 3, 40);
        assert_eq!(rows, vec![vec![0, 1]]);
    }

    #[test]
    fn pack_rows_wraps_when_separator_overflows() {
        // Row 0: 9 + 10 = 19; adding sep(3) + 10 = 32 > 30 → wrap.
        let rows = pack_rows(&[10, 10, 10], 9, 9, 3, 30);
        assert_eq!(rows, vec![vec![0], vec![1], vec![2]]);
    }

    #[test]
    fn pack_rows_packs_multiple_chunks_per_row() {
        // Row 0: 9 + 8 + 3 + 8 = 28 <= 30; next chunk would need 28 + 11 > 30.
        let rows = pack_rows(&[8, 8, 8, 8], 9, 9, 3, 30);
        assert_eq!(rows, vec![vec![0, 1], vec![2, 3]]);
    }

    #[test]
    fn pack_rows_gives_oversized_chunk_its_own_row() {
        let rows = pack_rows(&[100, 5], 9, 9, 3, 30);
        assert_eq!(rows, vec![vec![0], vec![1]]);
    }

    #[test]
    fn pack_rows_empty_input_yields_no_rows() {
        assert!(pack_rows(&[], 9, 9, 3, 30).is_empty());
    }

    #[test]
    fn shorten_path_keeps_ascii_path_within_width() {
        let shortened = shorten_path("/very/long/project/path", 14);

        assert!(shortened.width() <= 14);
        assert!(shortened.contains("..."));
        assert!(shortened.starts_with('/'));
        assert!(shortened.ends_with("path"));
    }

    #[test]
    fn shorten_path_handles_cjk_and_emoji_graphemes() {
        let shortened = shorten_path("/用户/项目/🚀/文件", 12);

        assert!(shortened.width() <= 12);
        assert!(shortened.contains("..."));
        assert!(shortened.ends_with("/文件"));
    }

    #[test]
    fn shorten_path_handles_narrow_widths() {
        assert_eq!(shorten_path("/用户/项目", 0), "");
        assert_eq!(shorten_path("/用户/项目", 1), ".");
        assert_eq!(shorten_path("/用户/项目", 3), "...");
    }
}
