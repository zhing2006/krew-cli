//! Agent response display: header labels, content indentation, and error display.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::custom_terminal;
use crate::render;
use crate::render::diff_render;

use super::App;

impl App {
    /// Look up the configured display color for an agent by name.
    /// Returns `None` when the name matches no configured agent (e.g. `all`).
    pub(crate) fn agent_color(&self, name: &str) -> Option<Color> {
        self.config
            .agents
            .iter()
            .find(|a| a.name == name)
            .map(|a| render::parse_color(&a.color))
    }

    /// Insert the agent header label: `[name] DisplayName:` (with optional lock icon for whisper).
    pub(crate) fn insert_agent_header(
        &self,
        terminal: &mut custom_terminal::Terminal,
        agent_name: &str,
        display_name: &str,
        color_name: &str,
        is_whisper: bool,
    ) -> anyhow::Result<()> {
        let color = render::parse_color(color_name);
        let colored = Style::default().fg(color).add_modifier(Modifier::BOLD);
        let plain = Style::default().add_modifier(Modifier::BOLD);

        let mut spans = vec![Span::styled(format!("[{agent_name}] "), colored)];
        if is_whisper {
            spans.push(Span::styled(
                "\u{1F512} ".to_string(), // 🔒
                Style::default().add_modifier(Modifier::BOLD),
            ));
        }
        spans.push(Span::styled(format!("{display_name}:"), plain));

        render::insert_lines(terminal, vec![Line::from(spans)])
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
        display: Vec<Span<'static>>,
    ) -> anyhow::Result<()> {
        let mut spans = vec![
            Span::raw("  "),
            Span::styled(prefix.to_string(), prefix_style),
        ];
        spans.extend(display);
        let line = Line::from(spans);

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

    /// Insert an approval decision feedback line in the scrollback.
    pub(crate) fn insert_decision_line(
        terminal: &mut custom_terminal::Terminal,
        decision: &krew_core::event::ReviewDecision,
    ) -> anyhow::Result<()> {
        use krew_core::event::ReviewDecision;

        let (symbol, text, color) = match decision {
            ReviewDecision::Approved => ("\u{2713}", "Approved", Color::Green),
            ReviewDecision::ApprovedForSession => {
                ("\u{2713}", "Approved (for session)", Color::Green)
            }
            ReviewDecision::Denied => ("\u{2717}", "Denied", Color::Red),
            ReviewDecision::Abort => ("\u{2717}", "Aborted", Color::Red),
        };

        let line = Line::from(Span::styled(
            format!("  {symbol} {text}"),
            Style::default().fg(color),
        ));
        terminal.insert_lines_above(vec![line])?;
        Ok(())
    }
}

/// Render a diff preview for write/edit tool calls.
///
/// For write_file: generates an all-additions diff from the content.
/// For edit_file: generates a unified diff from old_string → new_string.
/// Returns indented diff lines ready for `insert_lines_above()`.
pub(crate) fn render_tool_diff_preview(
    name: &str,
    arguments: &str,
    width: usize,
) -> Vec<Line<'static>> {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();
    let obj = match args.as_object() {
        Some(o) => o,
        None => return Vec::new(),
    };

    let (file_path, old, new) = match name {
        "write_file" => {
            let fp = obj.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let content = obj.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if content.is_empty() {
                return Vec::new();
            }
            (fp, "", content)
        }
        "edit_file" => {
            let fp = obj.get("file_path").and_then(|v| v.as_str()).unwrap_or("");
            let old_s = obj.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
            let new_s = obj.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
            if old_s.is_empty() && new_s.is_empty() {
                return Vec::new();
            }
            (fp, old_s, new_s)
        }
        _ => return Vec::new(),
    };

    let unified = similar::TextDiff::from_lines(old, new)
        .unified_diff()
        .header(&format!("a/{file_path}"), &format!("b/{file_path}"))
        .to_string();

    let diff_lines = diff_render::render_unified_diff(&unified, file_path, width.saturating_sub(4));

    // Indent each diff line by 4 spaces.
    diff_lines
        .into_iter()
        .map(|dl| {
            let mut spans = vec![Span::raw("    ")];
            spans.extend(dl.spans);
            Line::from(spans).style(dl.style)
        })
        .collect()
}

/// Parameters to skip in the tool call display line.
///
/// These are content parameters whose values are rendered separately
/// (e.g. as a diff preview below the tool call line).
fn is_content_param(tool_name: &str, param_name: &str) -> bool {
    matches!(
        (tool_name, param_name),
        ("write_file", "content") | ("edit_file", "old_string") | ("edit_file", "new_string")
    )
}

/// Format a tool call start display: `**read_file**("src/main.rs", offset=10)`
///
/// Returns styled spans with the tool name in bold.
/// Content parameters (write_file.content, edit_file.old_string/new_string)
/// are omitted — they are rendered as diff previews separately.
pub(crate) fn format_tool_call_display(name: &str, arguments: &str) -> Vec<Span<'static>> {
    let args: serde_json::Value = serde_json::from_str(arguments).unwrap_or_default();

    let params = match args.as_object() {
        Some(obj) => {
            let parts: Vec<String> = obj
                .iter()
                .filter(|(key, _)| !is_content_param(name, key))
                .map(|(key, val)| {
                    let display = match val {
                        serde_json::Value::String(s) => format!("\"{s}\""),
                        other => other.to_string(),
                    };
                    // First parameter shows value only, rest show key=value.
                    if obj.keys().find(|k| !is_content_param(name, k)) == Some(key) {
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

    // Convert MCP qualified names (mcp__server__tool) to display format (mcp:server/tool).
    let display =
        krew_tools::mcp::display_name_from_qualified(name).unwrap_or_else(|| name.to_string());

    vec![
        Span::styled(display, Style::default().add_modifier(Modifier::BOLD)),
        Span::raw(format!("({params})")),
    ]
}
