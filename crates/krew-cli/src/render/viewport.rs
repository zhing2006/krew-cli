//! TUI rendering logic — inline viewport with scrollback.
//!
//! Only the input prompt + status bar live inside the ratatui viewport.
//! All other content (header, messages) is inserted above the viewport
//! above the viewport, scrolling naturally into terminal history.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::app::App;
use crate::custom_terminal;

use super::popup;

/// Map a color name string to a ratatui `Color`.
pub fn parse_color(name: &str) -> Color {
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

/// Render the inline viewport: input prompt + status bar (or popup).
pub fn render_input_viewport(frame: &mut custom_terminal::Frame, app: &mut App) {
    let area = frame.area();

    let textarea_lines = app.textarea.lines().len() as u16;
    let input_height = textarea_lines.max(1);

    if app.popup.is_active() {
        // Popup replaces the status bar and may take multiple rows.
        let popup_lines = app.popup.render_lines(area.width);
        let popup_height = popup_lines.len() as u16;

        let chunks = Layout::vertical([
            Constraint::Length(1),            // Top separator
            Constraint::Length(input_height), // Input prompt
            Constraint::Length(1),            // Bottom separator
            Constraint::Length(popup_height), // Popup rows
        ])
        .split(area);

        render_separator(frame, chunks[0]);
        render_input(frame, app, chunks[1]);
        render_separator(frame, chunks[2]);
        popup::render_popup(frame, popup_lines, chunks[3]);
    } else {
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
