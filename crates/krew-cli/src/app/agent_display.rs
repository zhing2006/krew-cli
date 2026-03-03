//! Agent response display: header labels, content indentation, and error display.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::custom_terminal;
use crate::render;

use super::App;

impl App {
    /// Insert the agent header label: `[name] DisplayName:`
    pub(crate) fn insert_agent_header(
        &self,
        terminal: &mut custom_terminal::Terminal,
        agent_name: &str,
        display_name: &str,
        color_name: &str,
    ) -> anyhow::Result<()> {
        let color = render::parse_color(color_name);
        let colored = Style::default().fg(color).add_modifier(Modifier::BOLD);
        let plain = Style::default().add_modifier(Modifier::BOLD);

        let line = Line::from(vec![
            Span::styled(format!("[{agent_name}] "), colored),
            Span::styled(format!("{display_name}:"), plain),
        ]);

        render::insert_lines(terminal, vec![line])
    }

    /// Insert content lines with 2-space indentation and trailing blank.
    ///
    /// Use for final output (e.g. Done event, resume replay) where a
    /// trailing blank line is desired for visual spacing.
    pub(crate) fn insert_indented_lines(
        &self,
        terminal: &mut custom_terminal::Terminal,
        lines: Vec<Line<'static>>,
    ) -> anyhow::Result<()> {
        let indented = Self::indent_lines(lines);
        render::insert_lines(terminal, indented)
    }

    /// Insert streaming content lines with 2-space indentation (no trailing blank).
    ///
    /// Use during streaming where multiple batches form one logical block.
    /// The trailing blank is added only at the end (via `insert_indented_lines`).
    pub(crate) fn insert_indented_lines_streaming(
        &self,
        terminal: &mut custom_terminal::Terminal,
        lines: Vec<Line<'static>>,
    ) -> anyhow::Result<()> {
        let indented = Self::indent_lines(lines);
        terminal.insert_lines_above(indented)?;
        Ok(())
    }

    /// Insert thinking content lines in gray with 2-space indentation and trailing blank.
    pub(crate) fn insert_thinking_lines(
        &self,
        terminal: &mut custom_terminal::Terminal,
        lines: Vec<Line<'static>>,
    ) -> anyhow::Result<()> {
        let indented = Self::gray_indent_lines(lines);
        render::insert_lines(terminal, indented)
    }

    /// Insert streaming thinking content lines in gray (no trailing blank).
    pub(crate) fn insert_thinking_lines_streaming(
        &self,
        terminal: &mut custom_terminal::Terminal,
        lines: Vec<Line<'static>>,
    ) -> anyhow::Result<()> {
        let indented = Self::gray_indent_lines(lines);
        terminal.insert_lines_above(indented)?;
        Ok(())
    }

    /// Add 2-space indent to each line.
    fn indent_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
        lines
            .into_iter()
            .map(|line| {
                let mut spans = vec![Span::raw("  ".to_string())];
                spans.extend(line.spans);
                Line::from(spans)
            })
            .collect()
    }

    /// Add 2-space indent and override styles to gray.
    fn gray_indent_lines(lines: Vec<Line<'static>>) -> Vec<Line<'static>> {
        let gray = Style::default().fg(Color::DarkGray);
        lines
            .into_iter()
            .map(|line| {
                let mut spans = vec![Span::raw("  ".to_string())];
                for span in line.spans {
                    spans.push(Span::styled(span.content, gray));
                }
                Line::from(spans)
            })
            .collect()
    }

    /// Insert a tool display line with 2-space indent and dimmed text.
    ///
    /// The `prefix` and `prefix_style` control the leading symbol:
    /// - Start: yellow `⚡ `
    /// - Done:  dim `⎿  `
    pub(crate) fn insert_tool_line(
        &self,
        terminal: &mut custom_terminal::Terminal,
        prefix: &str,
        prefix_style: Style,
        display: &str,
    ) -> anyhow::Result<()> {
        let line = Line::from(vec![
            Span::raw("  "),
            Span::styled(prefix.to_string(), prefix_style),
            Span::raw(display.to_string()),
        ]);

        terminal.insert_lines_above(vec![line])?;
        Ok(())
    }

    /// Insert error line: `  ✗ {message}`
    pub(crate) fn insert_agent_error(
        &self,
        terminal: &mut custom_terminal::Terminal,
        message: &str,
    ) -> anyhow::Result<()> {
        let style = Style::default().fg(Color::Red).add_modifier(Modifier::BOLD);

        let line = Line::from(Span::styled(
            format!("  \u{2717} {message}"), // ✗
            style,
        ));

        render::insert_lines(terminal, vec![line])
    }
}

/// Format a tool call start display: `read_file("src/main.rs", offset=10)`
pub(crate) fn format_tool_call_display(name: &str, arguments: &str) -> String {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();

    let params = match args.as_object() {
        Some(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .map(|(key, val)| {
                    let display = match val {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        other => other.to_string(),
                    };
                    // First parameter shows value only, rest show key=value.
                    if obj.keys().next() == Some(key) {
                        display
                    } else {
                        format!("{key}={display}")
                    }
                })
                .collect();
            parts.join(", ")
        }
        None => String::new(),
    };

    format!("{name}({params})")
}

