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
        let style = Style::default().fg(color).add_modifier(Modifier::BOLD);

        let line = Line::from(vec![
            Span::styled(format!("[{agent_name}] "), style),
            Span::styled(format!("{display_name}:"), style),
        ]);

        render::insert_lines(terminal, vec![line])
    }

    /// Insert streaming content lines with 2-space indentation.
    pub(crate) fn insert_indented_lines(
        &self,
        terminal: &mut custom_terminal::Terminal,
        lines: Vec<Line<'static>>,
    ) -> anyhow::Result<()> {
        let indented: Vec<Line<'static>> = lines
            .into_iter()
            .map(|line| {
                let mut spans = vec![Span::raw("  ".to_string())];
                spans.extend(line.spans);
                Line::from(spans)
            })
            .collect();

        render::insert_lines(terminal, indented)
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
