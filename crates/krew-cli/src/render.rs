//! TUI rendering logic — inline viewport with scrollback.
//!
//! Only the input prompt + status bar live inside the ratatui viewport.
//! All other content (header, messages) is inserted above the viewport
//! via `insert_before`, scrolling naturally into terminal history.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use unicode_width::UnicodeWidthStr;

use crate::app::App;
use crate::custom_terminal;

/// Map a color name string to a ratatui `Color`.
fn parse_color(name: &str) -> Color {
    match name {
        "red" => Color::Red,
        "green" => Color::Green,
        "yellow" => Color::Yellow,
        "blue" => Color::Blue,
        "magenta" => Color::Magenta,
        "cyan" => Color::Cyan,
        "white" => Color::White,
        "gray" | "grey" => Color::Gray,
        "dark_gray" | "dark_grey" => Color::DarkGray,
        _ => Color::White,
    }
}

// ── Viewport rendering (input + status bar only) ───────────────────────

/// Render the inline viewport: input prompt + status bar.
pub fn render_input_viewport(frame: &mut custom_terminal::Frame, app: &mut App) {
    let area = frame.area();

    let textarea_lines = app.textarea.lines().len() as u16;
    let input_height = textarea_lines.max(1);

    let chunks = Layout::vertical([
        Constraint::Length(1),            // Top separator
        Constraint::Length(input_height), // Input prompt
        Constraint::Length(1),            // Bottom separator
        Constraint::Length(1),            // Status bar
    ])
    .split(area);

    render_separator(frame, chunks[0]);
    render_input(frame, app, chunks[1]);
    render_separator(frame, chunks[2]);
    render_status_bar(frame, app, chunks[3]);
}

/// Render the input prompt — `› ` prefix, no border.
fn render_input(frame: &mut custom_terminal::Frame, app: &mut App, area: Rect) {
    let chunks = Layout::horizontal([
        Constraint::Length(2), // `› `
        Constraint::Min(1),    // Textarea
    ])
    .split(area);

    let prompt = Paragraph::new(Line::from(Span::styled(
        "› ",
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD),
    )));
    frame.render_widget(prompt, chunks[0]);

    frame.render_widget(&app.textarea, chunks[1]);
}

/// Render a horizontal separator line.
fn render_separator(frame: &mut custom_terminal::Frame, area: Rect) {
    let line = "─".repeat(area.width as usize);
    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            line,
            Style::default().fg(Color::DarkGray),
        ))),
        area,
    );
}

/// Render the bottom status bar with config values.
fn render_status_bar(frame: &mut custom_terminal::Frame, app: &App, area: Rect) {
    let content = if let Some(hint) = &app.quit_hint {
        Line::from(Span::styled(
            format!("  {hint}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
    } else {
        let dir = app.cwd.display();
        let sep = Span::styled(" · ", Style::default().fg(Color::DarkGray));

        let mode_str = format!("{}", app.config.settings.approval_mode);

        let compact_str = match app.config.settings.auto_compact_threshold {
            Some(t) if t > 0 => format!("{}k", t / 1000),
            _ => "auto-compact off".to_string(),
        };

        Line::from(vec![
            Span::styled(
                format!("  {mode_str}"),
                Style::default().fg(Color::DarkGray),
            ),
            sep.clone(),
            Span::styled(compact_str, Style::default().fg(Color::DarkGray)),
            sep,
            Span::styled(format!("{dir}"), Style::default().fg(Color::DarkGray)),
        ])
    };

    frame.render_widget(Paragraph::new(content), area);
}

// ── Content inserted above viewport (scrolls into history) ─────────────

/// Shorten a path to fit within `max_len` by collapsing the middle with "...".
fn shorten_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len || max_len < 8 {
        return path.to_string();
    }
    // Keep the first and last segments, join with "...".
    let keep = (max_len - 3) / 2;
    let head = &path[..keep];
    let tail = &path[path.len() - (max_len - 3 - keep)..];
    format!("{head}...{tail}")
}

/// Insert the header box above the viewport.
pub fn insert_header(terminal: &mut custom_terminal::Terminal, app: &App) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
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
    // Line 1: " >_ Krew CLI (vX.Y.Z)"
    let line1_w = format!(" >_ Krew CLI (v{version})").len();
    // Line 2: " Agents: [name] Display | ..." + trailing space
    let agents_text_w: usize = agents_spans
        .iter()
        .map(|s| s.content.width())
        .sum::<usize>()
        + 1;
    // Line 3: " Directory: <path>  Type /help for commands "
    let dir_label = " Directory: ";
    let right_part = "Type /help for commands ";
    let line3_min_w = dir_label.len() + dir_full.len() + 2 + right_part.len();

    // Inner width = max of all lines; box width = inner + 2 borders.
    let content_w = line1_w.max(agents_text_w).max(line3_min_w);
    let screen_w = terminal.size().map(|s| s.width).unwrap_or(120);
    // Clamp: at least 40, at most terminal width.
    let box_width = ((content_w + 2) as u16).clamp(40, screen_w);

    // Header box: 3 content lines + 2 border lines = 5 rows.
    // Plus 1 blank line after = 6 total.
    let header_height = 6;

    terminal.insert_before(header_height, |buf| {
        let inner_w = box_width.saturating_sub(2) as usize;

        // Build the third line with path truncation based on actual available width.
        let max_path = inner_w
            .saturating_sub(dir_label.len())
            .saturating_sub(right_part.len())
            .saturating_sub(1);
        let dir = shorten_path(&dir_full, max_path);
        let left_len = dir_label.len() + dir.len();
        let gap = inner_w.saturating_sub(left_len + right_part.len());

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

/// Insert a chat message above the viewport.
///
/// `role` is `"you"` for user input, or an agent name for agent replies.
/// `color_name` is the agent's configured color string (ignored for user).
pub fn insert_message(
    terminal: &mut custom_terminal::Terminal,
    role: &str,
    content: &str,
    color_name: &str,
) -> anyhow::Result<()> {
    let (prefix_style, prefix_text): (Style, String) = if role == "you" {
        (
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "> ".to_string(),
        )
    } else {
        (
            Style::default()
                .fg(parse_color(color_name))
                .add_modifier(Modifier::BOLD),
            "\u{25cf} ".to_string(), // ●
        )
    };

    let mut lines: Vec<Line> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::styled(prefix_text.clone(), prefix_style),
                Span::raw(line.to_string()),
            ]));
        } else {
            let indent = " ".repeat(prefix_text.width());
            lines.push(Line::from(vec![
                Span::raw(indent),
                Span::raw(line.to_string()),
            ]));
        }
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
