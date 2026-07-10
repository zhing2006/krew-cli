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

/// Insert the header box above the viewport.
pub fn insert_header(terminal: &mut custom_terminal::Terminal, app: &App) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let session_id = &app.session_id;
    let dir_full = app.cwd.display().to_string();

    // Build the Agents line from config.
    let mut agents_spans = vec![Span::styled(
        " Agents: ",
        Style::default().fg(Color::DarkGray),
    )];

    for (i, name) in app.config.settings.reply_order.iter().enumerate() {
        if i > 0 {
            agents_spans.push(Span::styled(" | ", Style::default().fg(Color::DarkGray)));
        }
        if let Some(agent) = app.config.agents.iter().find(|a| &a.name == name) {
            let color = parse_color(&agent.color);
            agents_spans.push(Span::styled(
                format!("[{}]", agent.name),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ));
            agents_spans.push(Span::raw(format!(" {}", agent.display_name)));
        }
    }

    // Measure content widths to determine box width dynamically.
    // Line 1: " >_ Krew CLI (vX.Y.Z) — session_id"
    let line1_w = format!(" >_ Krew CLI (v{version}) — {session_id}").width();
    // Line 2: " Agents: [name] Display | ..." + trailing space
    let agents_text_w: usize = agents_spans
        .iter()
        .map(|s| s.content.width())
        .sum::<usize>()
        + 1;
    // Line 3: " Directory: <path>  Type /help for commands "
    let dir_label = " Directory: ";
    let right_part = "Type /help for commands ";
    let line3_min_w = dir_label.width() + dir_full.width() + 2 + right_part.width();

    // Inner width = max of all lines; box width = inner + 2 borders.
    let content_w = line1_w.max(agents_text_w).max(line3_min_w);
    let screen_w = terminal.size().map(|s| s.width).unwrap_or(120);
    // Clamp: at least 40, at most terminal width.
    let box_width = ((content_w + 2) as u16).clamp(40, screen_w);

    // Header box: 3 content lines + 2 border lines = 5 rows.
    // Plus 1 blank line after = 6 total.
    let header_height = 6;

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

        let lines = vec![
            Line::from(vec![
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
            ]),
            Line::from(agents_spans.clone()),
            Line::from(vec![
                Span::styled(dir_label.to_string(), Style::default().fg(Color::DarkGray)),
                Span::raw(dir),
                Span::raw(" ".repeat(gap)),
                Span::styled("Type ", Style::default().fg(Color::DarkGray)),
                Span::styled("/help", Style::default().fg(Color::Cyan)),
                Span::styled(" for commands ", Style::default().fg(Color::DarkGray)),
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::DarkGray));

        let area = Rect::new(0, 0, box_width, 5);
        let paragraph = Paragraph::new(lines).block(block);
        ratatui::widgets::Widget::render(paragraph, area, buf);
    })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::shorten_path;
    use unicode_width::UnicodeWidthStr;

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
