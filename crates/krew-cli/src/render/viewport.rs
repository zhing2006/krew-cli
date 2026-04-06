//! TUI rendering logic — inline viewport with scrollback.
//!
//! Only the input prompt + status bar live inside the ratatui viewport.
//! All other content (header, messages) is inserted above the viewport
//! above the viewport, scrolling naturally into terminal history.

use std::time::{Duration, Instant};

use ratatui::layout::{Constraint, Layout, Position, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use krew_core::router;

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

// ── Shimmer animation ───────────────────────────────────────────────

/// Generate shimmer (sweep) spans for the given text.
///
/// A bright band sweeps left-to-right across the text every `sweep_secs`
/// seconds. Each character's intensity is computed via a raised-cosine
/// falloff from the band center.
fn shimmer_spans(text: &str, base_color: Color, elapsed: Duration) -> Vec<Span<'static>> {
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return Vec::new();
    }

    let padding: usize = 10;
    let period = chars.len() + padding * 2;
    let sweep_secs: f32 = 2.0;
    let band_half_width: f32 = 5.0;

    // Sweep position normalized over the period.
    let pos_f = (elapsed.as_secs_f32() % sweep_secs) / sweep_secs * (period as f32);

    let mut spans = Vec::with_capacity(chars.len());

    for (i, ch) in chars.iter().enumerate() {
        let i_pos = (i + padding) as f32;
        let dist = (i_pos - pos_f).abs();

        // Raised cosine: 1.0 at center, smooth falloff to 0.0 at band edge.
        let intensity = if dist <= band_half_width {
            let x = std::f32::consts::PI * (dist / band_half_width);
            0.5 * (1.0 + x.cos())
        } else {
            0.0
        };

        let style = shimmer_style_for_level(intensity, base_color);
        spans.push(Span::styled(ch.to_string(), style));
    }
    spans
}

/// Map a shimmer intensity (0.0–1.0) to a terminal style.
///
/// Uses BOLD/DIM modifiers for broad compatibility (works without true-color
/// support). The base_color determines the foreground hue.
fn shimmer_style_for_level(intensity: f32, base_color: Color) -> Style {
    if intensity < 0.2 {
        Style::default().fg(base_color).add_modifier(Modifier::DIM)
    } else if intensity < 0.6 {
        Style::default().fg(base_color)
    } else {
        Style::default().fg(base_color).add_modifier(Modifier::BOLD)
    }
}

// ── Elapsed time formatting ──────────────────────────────────────────

/// Format elapsed seconds in compact form: `0s`, `45s`, `1m 23s`, `1h 05m`.
fn fmt_elapsed(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        format!("{m}m {s:02}s")
    } else {
        let h = secs / 3600;
        let m = (secs % 3600) / 60;
        format!("{h}h {m:02}m")
    }
}

// ── Viewport rendering (input + status bar only) ───────────────────────

/// Render the inline viewport: input prompt + status bar (or popup).
pub fn render_input_viewport(frame: &mut custom_terminal::Frame, app: &mut App) {
    let area = frame.area();

    // When an approval overlay is active, render it instead of the normal input.
    if let Some(overlay) = &app.approval_overlay {
        let overlay_height = overlay.desired_height();
        let has_status_line = app.agent_start_time.is_some();

        let mut constraints = Vec::new();
        if has_status_line {
            constraints.push(Constraint::Length(1)); // Agent status line
        }
        constraints.push(Constraint::Length(overlay_height)); // Approval overlay

        let chunks = Layout::vertical(constraints).split(area);
        let mut i = 0;

        if has_status_line {
            render_agent_status(frame, app, chunks[i]);
            i += 1;
        }
        overlay.render_widget(chunks[i], frame.buffer_mut());
        return;
    }

    // Use visual line count (after word wrapping) for layout.
    let textarea_width = area.width.saturating_sub(2); // minus prompt "› "
    let textarea_lines = app.textarea.desired_height(textarea_width.max(1));
    let input_height = textarea_lines.max(1);

    let has_status_line = app.agent_start_time.is_some();

    let pending_h = pending_area_height(app);

    if app.popup.is_active() {
        // Popup replaces the status bar and may take multiple rows.
        let popup_lines = app.popup.render_lines(area.width);
        let popup_height = popup_lines.len() as u16;

        let mut constraints = Vec::new();
        if has_status_line {
            constraints.push(Constraint::Length(1)); // Agent status line
        }
        // pending_h is 0 when popup is active (hidden by pending_area_height).
        if pending_h > 0 {
            constraints.push(Constraint::Length(pending_h));
        }
        constraints.push(Constraint::Length(1)); // Top separator
        constraints.push(Constraint::Length(input_height)); // Input prompt
        constraints.push(Constraint::Length(1)); // Bottom separator
        constraints.push(Constraint::Length(popup_height)); // Popup rows

        let chunks = Layout::vertical(constraints).split(area);
        let mut i = 0;

        if has_status_line {
            render_agent_status(frame, app, chunks[i]);
            i += 1;
        }
        if pending_h > 0 {
            render_pending_area(frame, app, chunks[i]);
            i += 1;
        }
        render_separator(frame, chunks[i]);
        render_input(frame, app, chunks[i + 1]);
        render_separator(frame, chunks[i + 2]);
        popup::render_popup(frame, popup_lines, chunks[i + 3]);
    } else {
        let mut constraints = Vec::new();
        if has_status_line {
            constraints.push(Constraint::Length(1)); // Agent status line
        }
        if pending_h > 0 {
            constraints.push(Constraint::Length(pending_h)); // Pending area
        }
        constraints.push(Constraint::Length(1)); // Top separator
        constraints.push(Constraint::Length(input_height)); // Input prompt
        constraints.push(Constraint::Length(1)); // Bottom separator
        constraints.push(Constraint::Length(1)); // Status bar

        let chunks = Layout::vertical(constraints).split(area);
        let mut i = 0;

        if has_status_line {
            render_agent_status(frame, app, chunks[i]);
            i += 1;
        }
        if pending_h > 0 {
            render_pending_area(frame, app, chunks[i]);
            i += 1;
        }
        render_separator(frame, chunks[i]);
        render_input(frame, app, chunks[i + 1]);
        render_separator(frame, chunks[i + 2]);
        render_status_bar(frame, app, chunks[i + 3]);
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

    // Show real terminal cursor at the textarea cursor position.
    if let Some((cx, cy)) = app.textarea.cursor_pos_in(chunks[1]) {
        frame.set_cursor_position(Position::new(cx, cy));
    }
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

/// Render the agent status indicator line with shimmer animation.
///
/// Layout: `  ● AgentName Working  (12s · ESC to interrupt)`
///
/// The "Working" text uses a shimmer sweep effect — a bright band travels
/// left-to-right every 2 seconds, giving the impression of ongoing activity.
fn render_agent_status(frame: &mut custom_terminal::Frame, app: &App, area: Rect) {
    let start = match app.agent_start_time {
        Some(t) => t,
        None => return,
    };

    let elapsed = Instant::now().duration_since(start);
    let elapsed_str = fmt_elapsed(elapsed.as_secs());

    let agent_color = app
        .agent_color
        .as_deref()
        .map(parse_color)
        .unwrap_or(Color::White);

    let display_name = app.agent_display_name.as_deref().unwrap_or("Agent");

    // Blink spinner: alternate between bright and dim every 600ms.
    let blink_on = (elapsed.as_millis() / 600).is_multiple_of(2);
    let spinner = if blink_on {
        Span::styled("●", Style::default().fg(agent_color))
    } else {
        Span::styled("◦", Style::default().fg(Color::DarkGray))
    };

    let dim = Style::default().fg(Color::DarkGray);

    // Build the line: prefix + spinner + shimmer text + elapsed/hint suffix.
    let mut spans = vec![Span::raw("  "), spinner, Span::raw(" ")];

    // Shimmer sweep across status text (default "Working", or retry info).
    let shimmer_text = match &app.agent_status_text {
        Some(text) => format!("{display_name} {text}"),
        None => format!("{display_name} Working"),
    };
    spans.extend(shimmer_spans(&shimmer_text, agent_color, elapsed));

    // Elapsed time and interrupt hint (static dim style).
    spans.push(Span::styled(format!("  ({elapsed_str} · "), dim));
    spans.push(Span::styled(
        "ESC",
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    ));
    spans.push(Span::styled(" to interrupt)", dim));

    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

/// Compute the height of the pending message area (0 if empty or overlay/popup active).
pub fn pending_area_height(app: &App) -> u16 {
    if app.pending_messages.is_empty() {
        return 0;
    }
    // Hide pending area when overlay or popup is active.
    if app.approval_overlay.is_some() || app.popup.is_active() {
        return 0;
    }
    // 1 title line + N message lines
    1 + app.pending_messages.len() as u16
}

/// Render the pending message area at the top of the viewport.
fn render_pending_area(frame: &mut custom_terminal::Frame, app: &App, area: Rect) {
    if area.height == 0 {
        return;
    }

    let width = area.width as usize;

    // Split area: first row = title, remaining = messages.
    let constraints: Vec<Constraint> = std::iter::once(Constraint::Length(1))
        .chain(
            app.pending_messages
                .iter()
                .map(|_| Constraint::Length(1)),
        )
        .collect();
    let chunks = Layout::vertical(constraints).split(area);

    // Title line: ┄ Pending (N) ┄
    let title_text = format!(" Pending ({}) ", app.pending_messages.len());
    let pad_total = width.saturating_sub(title_text.len());
    let pad_left = pad_total / 2;
    let pad_right = pad_total - pad_left;
    let title_line = Line::from(Span::styled(
        format!(
            "{}{}{}",
            "┄".repeat(pad_left.min(20)),
            title_text,
            "┄".repeat(pad_right.min(20))
        ),
        Style::default().fg(Color::DarkGray),
    ));
    frame.render_widget(Paragraph::new(title_line), chunks[0]);

    // Message lines.
    let agent_names: Vec<String> = app.config.agents.iter().map(|a| a.name.clone()).collect();
    for (idx, pending) in app.pending_messages.iter().enumerate() {
        let mut spans: Vec<Span<'static>> = Vec::new();

        // ⏳ prefix
        spans.push(Span::styled(
            "⏳ ".to_string(),
            Style::default().fg(Color::DarkGray),
        ));

        // Parse for routing dots preview.
        if let Ok((addressee, _, is_whisper)) =
            router::parse_input(&pending.raw_input, &agent_names)
        {
            if is_whisper {
                spans.push(Span::styled(
                    "\u{1F512}".to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
            }

            let available: std::collections::HashSet<String> =
                app.agents.keys().cloned().collect();
            let target_names = router::resolve_target_names(
                &addressee,
                &app.config.settings.reply_order,
                &available,
                None,
            );
            for name in &target_names {
                let color = app
                    .config
                    .agents
                    .iter()
                    .find(|a| a.name == *name)
                    .map(|a| parse_color(&a.color))
                    .unwrap_or(Color::White);
                spans.push(Span::styled(
                    "\u{25cf}".to_string(),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ));
            }
            if !target_names.is_empty() {
                spans.push(Span::raw(" ".to_string()));
            }
        }

        // Message text — first line only, truncated.
        let first_line = pending
            .raw_input
            .lines()
            .next()
            .unwrap_or(&pending.raw_input);
        let is_multiline = pending.raw_input.contains('\n');

        // Calculate remaining width for text.
        let prefix_width: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let max_text_width = width.saturating_sub(prefix_width).saturating_sub(1);

        let display_text = if first_line.chars().count() > max_text_width || is_multiline {
            let truncated: String = first_line.chars().take(max_text_width.saturating_sub(1)).collect();
            format!("{truncated}…")
        } else {
            first_line.to_string()
        };

        spans.push(Span::styled(display_text, Style::default().fg(Color::Gray)));

        frame.render_widget(Paragraph::new(Line::from(spans)), chunks[idx + 1]);
    }
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
        let mode_color = match app.config.settings.approval_mode {
            krew_config::ApprovalMode::Suggest => Color::DarkGray,
            krew_config::ApprovalMode::AutoEdit => Color::Yellow,
            krew_config::ApprovalMode::FullAuto => Color::Red,
        };

        let (compact_str, compact_color) = match app.config.settings.auto_compact_threshold {
            Some(t) if t > 0 => (format!("auto-compact {}k", t / 1000), Color::Cyan),
            _ => ("auto-compact off".to_string(), Color::DarkGray),
        };

        Line::from(vec![
            Span::styled(format!("  {mode_str}"), Style::default().fg(mode_color)),
            sep.clone(),
            Span::styled(compact_str, Style::default().fg(compact_color)),
            sep,
            Span::styled(format!("{dir}"), Style::default().fg(Color::DarkGray)),
        ])
    };

    frame.render_widget(Paragraph::new(content), area);
}
