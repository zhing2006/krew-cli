//! Syntax highlighting for fenced code blocks using syntect + two-face.

use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use std::sync::OnceLock;
use syntect::highlighting::{FontStyle, Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Maximum code block size before falling back to plain text.
const MAX_CODE_BYTES: usize = 512 * 1024; // 512 KB
const MAX_CODE_LINES: usize = 10_000;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME: OnceLock<Theme> = OnceLock::new();

fn syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines)
}

fn theme() -> &'static Theme {
    THEME.get_or_init(|| {
        let theme_set = ThemeSet::load_defaults();
        // Use "base16-ocean.dark" as default theme — good terminal contrast.
        theme_set
            .themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| {
                theme_set
                    .themes
                    .values()
                    .next()
                    .cloned()
                    .expect("at least one theme available")
            })
    })
}

/// Highlight a code block and return ratatui Lines.
///
/// Falls back to plain monospace rendering when:
/// - The code exceeds size limits (512KB or 10k lines)
/// - The language is not recognized by syntect
/// - No language is specified
pub fn highlight_code_to_lines(code: &str, lang: Option<&str>) -> Vec<Line<'static>> {
    // Safety guard for pathologically large code blocks.
    if code.len() > MAX_CODE_BYTES || code.lines().count() > MAX_CODE_LINES {
        return plain_code_lines(code);
    }

    let ss = syntax_set();

    let syntax = lang
        .and_then(|l| ss.find_syntax_by_token(l))
        .or_else(|| lang.and_then(|l| ss.find_syntax_by_extension(l)));

    let syntax = match syntax {
        Some(s) => s,
        None => return plain_code_lines(code),
    };

    let theme = theme();
    let mut highlighter = syntect::easy::HighlightLines::new(syntax, theme);

    code.lines()
        .map(|line| {
            let regions = highlighter.highlight_line(line, ss).unwrap_or_default();

            let spans: Vec<Span<'static>> = regions
                .into_iter()
                .map(|(style, text)| {
                    let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                    let mut ratatui_style = Style::new().fg(fg);
                    if style.font_style.contains(FontStyle::BOLD) {
                        ratatui_style = ratatui_style.bold();
                    }
                    if style.font_style.contains(FontStyle::ITALIC) {
                        ratatui_style = ratatui_style.italic();
                    }
                    if style.font_style.contains(FontStyle::UNDERLINE) {
                        ratatui_style = ratatui_style.underlined();
                    }
                    Span::styled(text.to_string(), ratatui_style)
                })
                .collect();

            Line::from(spans)
        })
        .collect()
}

/// Render code as plain monospace text (cyan foreground).
fn plain_code_lines(code: &str) -> Vec<Line<'static>> {
    let style = Style::new().fg(Color::Cyan);
    code.lines()
        .map(|line| Line::from(Span::styled(line.to_string(), style)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_highlight_rust_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let lines = highlight_code_to_lines(code, Some("rust"));
        assert_eq!(lines.len(), 3);
        // Each line should have styled spans (not plain).
        assert!(!lines[0].spans.is_empty());
    }

    #[test]
    fn test_highlight_unknown_language_fallback() {
        let code = "some code here";
        let lines = highlight_code_to_lines(code, Some("nonexistent_lang_xyz"));
        assert_eq!(lines.len(), 1);
        // Should fallback to plain cyan.
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_highlight_no_language_fallback() {
        let code = "some code";
        let lines = highlight_code_to_lines(code, None);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_highlight_large_code_fallback() {
        let code = "x\n".repeat(MAX_CODE_LINES + 1);
        let lines = highlight_code_to_lines(&code, Some("rust"));
        // Should fallback to plain — check first line is cyan.
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Cyan));
    }
}
