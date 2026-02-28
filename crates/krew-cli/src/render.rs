//! TUI rendering logic — inline viewport with scrollback.
//!
//! Only the input prompt + status bar live inside the ratatui viewport.
//! All other content (header, messages) is inserted above the viewport
//! via `insert_before`, scrolling naturally into terminal history.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::app::App;
use crate::custom_terminal;

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

/// Render the bottom status bar.
fn render_status_bar(frame: &mut custom_terminal::Frame, app: &App, area: Rect) {
    let content = if let Some(hint) = &app.quit_hint {
        Line::from(Span::styled(
            format!("  {hint}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        ))
    } else {
        let dir = app.cwd.display();
        let sep = Span::styled(" · ", Style::default().fg(Color::DarkGray));
        Line::from(vec![
            Span::styled("  suggest", Style::default().fg(Color::DarkGray)),
            sep.clone(),
            Span::styled("auto-compact off", Style::default().fg(Color::DarkGray)),
            sep,
            Span::styled(format!("{dir}"), Style::default().fg(Color::DarkGray)),
        ])
    };

    frame.render_widget(Paragraph::new(content), area);
}

// ── Content inserted above viewport (scrolls into history) ─────────────

/// Insert the header box above the viewport.
pub fn insert_header(terminal: &mut custom_terminal::Terminal, app: &App) -> anyhow::Result<()> {
    let version = env!("CARGO_PKG_VERSION");
    let dir = app.cwd.display();

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
        Line::from(vec![
            Span::styled(" Directory: ", Style::default().fg(Color::DarkGray)),
            Span::raw(format!("{dir}")),
        ]),
        Line::from(vec![
            Span::styled(" Type ", Style::default().fg(Color::DarkGray)),
            Span::styled("/help", Style::default().fg(Color::Cyan)),
            Span::styled(" for commands", Style::default().fg(Color::DarkGray)),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    // Header box: 3 content lines + 2 border lines = 5 rows.
    // Plus 1 blank line after = 6 total.
    let header_height = 6;

    terminal.insert_before(header_height, |buf| {
        let area = Rect::new(0, 0, 50.min(buf.area.width), 5);
        let paragraph = Paragraph::new(lines).block(block);
        ratatui::widgets::Widget::render(paragraph, area, buf);
    })?;

    Ok(())
}

/// Insert a chat message above the viewport.
pub fn insert_message(
    terminal: &mut custom_terminal::Terminal,
    prefix: &str,
    content: &str,
) -> anyhow::Result<()> {
    let (prefix_style, prefix_text): (Style, String) = match prefix {
        "you" => (
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
            "you> ".to_string(),
        ),
        "echo" => (
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            "echo> ".to_string(),
        ),
        other => (
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
            format!("[{other}] "),
        ),
    };

    let mut lines: Vec<Line> = Vec::new();
    for (i, line) in content.lines().enumerate() {
        if i == 0 {
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(prefix_text.clone(), prefix_style),
                Span::raw(line.to_string()),
            ]));
        } else {
            let indent = " ".repeat(2 + prefix_text.len());
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
