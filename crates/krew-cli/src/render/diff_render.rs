//! Renders unified diffs with line numbers, gutter signs, and optional syntax
//! highlighting.
//!
//! Each diff line is prefixed by a right-aligned line number, a gutter sign
//! (`+` / `-` / ` `), and the content text. When a recognized file extension
//! is present, the content text is syntax-highlighted.
//!
//! **Theme-aware styling:** diff backgrounds adapt to the terminal's
//! background lightness. Dark terminals get muted tints; light terminals get
//! GitHub-style pastels. The renderer uses fixed palettes for
//! TrueColor / 256-color / 16-color terminals.

use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line as RtLine, Span as RtSpan};
use unicode_width::UnicodeWidthChar;

use super::color::is_light;
use super::terminal_palette::{
    StdoutColorLevel, default_bg, indexed_color, rgb_color, stdout_color_level,
};

/// Display width of a tab character in columns.
const TAB_WIDTH: usize = 4;

// -- Diff background palette -------------------------------------------------

// Dark-theme truecolor palette.
const DARK_TC_ADD_LINE_BG_RGB: (u8, u8, u8) = (33, 58, 43); // #213A2B
const DARK_TC_DEL_LINE_BG_RGB: (u8, u8, u8) = (74, 34, 29); // #4A221D

// Light-theme truecolor palette (GitHub-style).
const LIGHT_TC_ADD_LINE_BG_RGB: (u8, u8, u8) = (218, 251, 225); // #dafbe1
const LIGHT_TC_DEL_LINE_BG_RGB: (u8, u8, u8) = (255, 235, 233); // #ffebe9
const LIGHT_TC_ADD_NUM_BG_RGB: (u8, u8, u8) = (172, 238, 187); // #aceebb
const LIGHT_TC_DEL_NUM_BG_RGB: (u8, u8, u8) = (255, 206, 203); // #ffcecb
const LIGHT_TC_GUTTER_FG_RGB: (u8, u8, u8) = (31, 35, 40); // #1f2328

// 256-color palette.
const DARK_256_ADD_LINE_BG_IDX: u8 = 22;
const DARK_256_DEL_LINE_BG_IDX: u8 = 52;
const LIGHT_256_ADD_LINE_BG_IDX: u8 = 194;
const LIGHT_256_DEL_LINE_BG_IDX: u8 = 224;
const LIGHT_256_ADD_NUM_BG_IDX: u8 = 157;
const LIGHT_256_DEL_NUM_BG_IDX: u8 = 217;
const LIGHT_256_GUTTER_FG_IDX: u8 = 236;

// -- Core types ---------------------------------------------------------------

/// Classifies a diff line for gutter sign rendering and style selection.
#[derive(Clone, Copy)]
pub enum DiffLineType {
    Insert,
    Delete,
    Context,
}

/// Controls which color palette the diff renderer uses.
#[derive(Clone, Copy, Debug)]
enum DiffTheme {
    Dark,
    Light,
}

/// Palette depth the diff renderer will target.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DiffColorLevel {
    TrueColor,
    Ansi256,
    Ansi16,
}

/// Subset of DiffColorLevel that supports tinted backgrounds.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RichDiffColorLevel {
    TrueColor,
    Ansi256,
}

impl RichDiffColorLevel {
    fn from_diff_color_level(level: DiffColorLevel) -> Option<Self> {
        match level {
            DiffColorLevel::TrueColor => Some(Self::TrueColor),
            DiffColorLevel::Ansi256 => Some(Self::Ansi256),
            DiffColorLevel::Ansi16 => None,
        }
    }
}

/// Pre-resolved background colors for insert and delete diff lines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ResolvedDiffBackgrounds {
    add: Option<Color>,
    del: Option<Color>,
}

/// Precomputed render state for diff line styling.
#[derive(Clone, Copy, Debug)]
pub struct DiffRenderStyleContext {
    theme: DiffTheme,
    color_level: DiffColorLevel,
    diff_backgrounds: ResolvedDiffBackgrounds,
}

// -- Context construction -----------------------------------------------------

/// Snapshot the current terminal environment into a reusable style context.
pub fn current_diff_render_style_context() -> DiffRenderStyleContext {
    let theme = diff_theme();
    let color_level = diff_color_level();
    let diff_backgrounds = fallback_diff_backgrounds(theme, color_level);
    DiffRenderStyleContext {
        theme,
        color_level,
        diff_backgrounds,
    }
}

fn diff_theme() -> DiffTheme {
    match default_bg() {
        Some(rgb) if is_light(rgb) => DiffTheme::Light,
        _ => DiffTheme::Dark,
    }
}

fn diff_color_level() -> DiffColorLevel {
    // Windows Terminal promotion: WT_SESSION present → TrueColor
    if std::env::var_os("WT_SESSION").is_some() {
        return DiffColorLevel::TrueColor;
    }
    match stdout_color_level() {
        StdoutColorLevel::TrueColor => DiffColorLevel::TrueColor,
        StdoutColorLevel::Ansi256 => DiffColorLevel::Ansi256,
        StdoutColorLevel::Ansi16 => DiffColorLevel::Ansi16,
    }
}

fn fallback_diff_backgrounds(
    theme: DiffTheme,
    color_level: DiffColorLevel,
) -> ResolvedDiffBackgrounds {
    match RichDiffColorLevel::from_diff_color_level(color_level) {
        Some(level) => ResolvedDiffBackgrounds {
            add: Some(add_line_bg(theme, level)),
            del: Some(del_line_bg(theme, level)),
        },
        None => ResolvedDiffBackgrounds::default(),
    }
}

// -- Public API ---------------------------------------------------------------

/// Render a unified diff string into colored ratatui Lines.
///
/// This is the main entry point for rendering diffs in the TUI. It parses
/// the unified diff, applies syntax highlighting when possible, and returns
/// styled lines ready for display via `insert_widget_above()`.
pub fn render_unified_diff(
    unified_diff: &str,
    file_path: &str,
    width: usize,
) -> Vec<RtLine<'static>> {
    let mut out = Vec::new();
    let style_context = current_diff_render_style_context();

    let Ok(patch) = diffy::Patch::from_str(unified_diff) else {
        // Fallback: render as plain text if patch parsing fails.
        for line in unified_diff.lines() {
            out.push(RtLine::from(line.to_string()));
        }
        return out;
    };

    // Calculate max line number for gutter width.
    let mut max_line_number: usize = 0;
    let mut total_diff_bytes: usize = 0;
    let mut total_diff_lines: usize = 0;
    for h in patch.hunks() {
        let mut old_ln = h.old_range().start();
        let mut new_ln = h.new_range().start();
        for l in h.lines() {
            let text = match l {
                diffy::Line::Insert(t) | diffy::Line::Delete(t) | diffy::Line::Context(t) => t,
            };
            total_diff_bytes += text.len();
            total_diff_lines += 1;
            match l {
                diffy::Line::Insert(_) => {
                    max_line_number = max_line_number.max(new_ln);
                    new_ln += 1;
                }
                diffy::Line::Delete(_) => {
                    max_line_number = max_line_number.max(old_ln);
                    old_ln += 1;
                }
                diffy::Line::Context(_) => {
                    max_line_number = max_line_number.max(new_ln);
                    old_ln += 1;
                    new_ln += 1;
                }
            }
        }
    }

    // Detect language for syntax highlighting from file extension.
    let lang = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str());

    // Skip syntax highlighting for large diffs.
    let highlight_lang = if total_diff_bytes > 512 * 1024 || total_diff_lines > 10_000 {
        None
    } else {
        lang
    };

    let ln_width = line_number_width(max_line_number);
    let mut is_first_hunk = true;

    for h in patch.hunks() {
        if !is_first_hunk {
            // Hunk separator.
            let spacer = format!("{:width$} ", "", width = ln_width.max(1));
            let spacer_span = RtSpan::styled(
                spacer,
                style_gutter_for(
                    DiffLineType::Context,
                    style_context.theme,
                    style_context.color_level,
                ),
            );
            out.push(RtLine::from(vec![spacer_span, "⋮".dim()]));
        }
        is_first_hunk = false;

        // Highlight each hunk as a single block to preserve parser state.
        let hunk_syntax_lines = highlight_lang.and_then(|language| {
            let hunk_text: String = h
                .lines()
                .iter()
                .map(|line| {
                    let text = match line {
                        diffy::Line::Insert(t)
                        | diffy::Line::Delete(t)
                        | diffy::Line::Context(t) => *t,
                    };
                    // Ensure trailing newline so lines don't merge during
                    // concatenation (edit_file fragments often lack one).
                    if text.ends_with('\n') {
                        text.to_string()
                    } else {
                        format!("{text}\n")
                    }
                })
                .collect();
            let syntax_lines = highlight_code_to_styled_spans(&hunk_text, language)?;
            (syntax_lines.len() == h.lines().len()).then_some(syntax_lines)
        });

        let mut old_ln = h.old_range().start();
        let mut new_ln = h.new_range().start();
        for (line_idx, l) in h.lines().iter().enumerate() {
            let syntax_spans = hunk_syntax_lines.as_ref().and_then(|sl| sl.get(line_idx));
            match l {
                diffy::Line::Insert(text) => {
                    let s = text.trim_end_matches('\n');
                    let p = DiffLineParams {
                        line_number: new_ln,
                        kind: DiffLineType::Insert,
                        text: s,
                        width,
                        line_number_width: ln_width,
                        syntax_spans,
                    };
                    out.extend(render_diff_line(&p, &style_context));
                    new_ln += 1;
                }
                diffy::Line::Delete(text) => {
                    let s = text.trim_end_matches('\n');
                    let p = DiffLineParams {
                        line_number: old_ln,
                        kind: DiffLineType::Delete,
                        text: s,
                        width,
                        line_number_width: ln_width,
                        syntax_spans,
                    };
                    out.extend(render_diff_line(&p, &style_context));
                    old_ln += 1;
                }
                diffy::Line::Context(text) => {
                    let s = text.trim_end_matches('\n');
                    let p = DiffLineParams {
                        line_number: new_ln,
                        kind: DiffLineType::Context,
                        text: s,
                        width,
                        line_number_width: ln_width,
                        syntax_spans,
                    };
                    out.extend(render_diff_line(&p, &style_context));
                    old_ln += 1;
                    new_ln += 1;
                }
            }
        }
    }

    out
}

fn line_number_width(max_line_number: usize) -> usize {
    if max_line_number == 0 {
        1
    } else {
        max_line_number.to_string().len()
    }
}

// -- Internal rendering -------------------------------------------------------

/// Parameters for rendering a single diff line.
struct DiffLineParams<'a> {
    line_number: usize,
    kind: DiffLineType,
    text: &'a str,
    width: usize,
    line_number_width: usize,
    syntax_spans: Option<&'a Vec<RtSpan<'static>>>,
}

/// Render a single diff line with gutter, sign, and content.
fn render_diff_line(p: &DiffLineParams<'_>, ctx: &DiffRenderStyleContext) -> Vec<RtLine<'static>> {
    let ln_str = p.line_number.to_string();
    let gutter_width = p.line_number_width.max(1);
    let prefix_cols = gutter_width + 1;

    let (sign_char, sign_style, content_style) = match p.kind {
        DiffLineType::Insert => (
            '+',
            style_sign_add(ctx.theme, ctx.color_level, ctx.diff_backgrounds),
            style_add(ctx.theme, ctx.color_level, ctx.diff_backgrounds),
        ),
        DiffLineType::Delete => (
            '-',
            style_sign_del(ctx.theme, ctx.color_level, ctx.diff_backgrounds),
            style_del(ctx.theme, ctx.color_level, ctx.diff_backgrounds),
        ),
        DiffLineType::Context => (' ', style_context(), style_context()),
    };

    let line_bg = style_line_bg_for(p.kind, ctx.diff_backgrounds);
    let gutter_style = style_gutter_for(p.kind, ctx.theme, ctx.color_level);

    let available_content_cols = p.width.saturating_sub(prefix_cols + 1).max(1);

    // When we have syntax spans, compose them with the diff style.
    if let Some(syn_spans) = p.syntax_spans {
        let gutter = format!("{ln_str:>gutter_width$} ");
        let sign = format!("{sign_char}");
        let styled: Vec<RtSpan<'static>> = syn_spans
            .iter()
            .map(|sp| {
                let style = if matches!(p.kind, DiffLineType::Delete) {
                    sp.style.add_modifier(Modifier::DIM)
                } else {
                    sp.style
                };
                RtSpan::styled(sp.content.clone().into_owned(), style)
            })
            .collect();

        let wrapped_chunks = wrap_styled_spans(&styled, available_content_cols);

        let mut lines: Vec<RtLine<'static>> = Vec::new();
        for (i, chunk) in wrapped_chunks.into_iter().enumerate() {
            let mut row_spans: Vec<RtSpan<'static>> = Vec::new();
            if i == 0 {
                row_spans.push(RtSpan::styled(gutter.clone(), gutter_style));
                row_spans.push(RtSpan::styled(sign.clone(), sign_style));
            } else {
                let cont_gutter = format!("{:gutter_width$}  ", "");
                row_spans.push(RtSpan::styled(cont_gutter, gutter_style));
            }
            row_spans.extend(chunk);
            lines.push(RtLine::from(row_spans).style(line_bg));
        }
        return lines;
    }

    // Plain text path.
    let styled = vec![RtSpan::styled(p.text.to_string(), content_style)];
    let wrapped_chunks = wrap_styled_spans(&styled, available_content_cols);

    let mut lines: Vec<RtLine<'static>> = Vec::new();
    for (i, chunk) in wrapped_chunks.into_iter().enumerate() {
        let mut row_spans: Vec<RtSpan<'static>> = Vec::new();
        if i == 0 {
            let gutter = format!("{ln_str:>gutter_width$} ");
            let sign = format!("{sign_char}");
            row_spans.push(RtSpan::styled(gutter, gutter_style));
            row_spans.push(RtSpan::styled(sign, sign_style));
        } else {
            let cont_gutter = format!("{:gutter_width$}  ", "");
            row_spans.push(RtSpan::styled(cont_gutter, gutter_style));
        }
        row_spans.extend(chunk);
        lines.push(RtLine::from(row_spans).style(line_bg));
    }

    lines
}

// -- Syntax highlighting bridge -----------------------------------------------

/// Highlight code and return per-line styled spans for diff integration.
///
/// Uses our existing highlight infrastructure. Returns None when the language
/// is unrecognized or the input exceeds guardrails.
fn highlight_code_to_styled_spans(code: &str, lang: &str) -> Option<Vec<Vec<RtSpan<'static>>>> {
    use std::sync::OnceLock;
    use syntect::easy::HighlightLines;
    use syntect::highlighting::{FontStyle, Theme, ThemeSet};
    use syntect::parsing::SyntaxSet;
    use syntect::util::LinesWithEndings;

    static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
    static THEME: OnceLock<Theme> = OnceLock::new();

    let ss = SYNTAX_SET.get_or_init(two_face::syntax::extra_newlines);
    let theme = THEME.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes
            .get("base16-ocean.dark")
            .cloned()
            .unwrap_or_else(|| ts.themes.values().next().cloned().expect("theme"))
    });

    if code.is_empty() || code.len() > 512 * 1024 || code.lines().count() > 10_000 {
        return None;
    }

    // Try common language aliases.
    let patched = match lang {
        "csharp" | "c-sharp" => "c#",
        "golang" => "go",
        "python3" => "python",
        "shell" => "bash",
        _ => lang,
    };

    let syntax = ss
        .find_syntax_by_token(patched)
        .or_else(|| ss.find_syntax_by_name(patched))
        .or_else(|| {
            let lower = patched.to_ascii_lowercase();
            ss.syntaxes()
                .iter()
                .find(|s| s.name.to_ascii_lowercase() == lower)
        })
        .or_else(|| ss.find_syntax_by_extension(lang))?;

    let mut h = HighlightLines::new(syntax, theme);
    let mut lines: Vec<Vec<RtSpan<'static>>> = Vec::new();

    for line in LinesWithEndings::from(code) {
        let ranges = h.highlight_line(line, ss).ok()?;
        let mut spans: Vec<RtSpan<'static>> = Vec::new();
        for (style, text) in ranges {
            let text = text.trim_end_matches(['\n', '\r']);
            if text.is_empty() {
                continue;
            }
            let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
            let mut rt_style = Style::new().fg(fg);
            if style.font_style.contains(FontStyle::BOLD) {
                rt_style = rt_style.bold();
            }
            // Skip italic/underline (distracting in terminal).
            spans.push(RtSpan::styled(text.to_string(), rt_style));
        }
        if spans.is_empty() {
            spans.push(RtSpan::raw(String::new()));
        }
        lines.push(spans);
    }

    Some(lines)
}

// -- Line wrapping ------------------------------------------------------------

/// Split styled spans into chunks that fit within `max_cols` display columns.
///
/// Styles are preserved across split boundaries. Tabs are expanded to
/// TAB_WIDTH columns. CJK characters (width 2) are handled correctly.
fn wrap_styled_spans(spans: &[RtSpan<'static>], max_cols: usize) -> Vec<Vec<RtSpan<'static>>> {
    let mut result: Vec<Vec<RtSpan<'static>>> = Vec::new();
    let mut current_line: Vec<RtSpan<'static>> = Vec::new();
    let mut col: usize = 0;

    for span in spans {
        let style = span.style;
        let text = span.content.as_ref();
        let mut remaining = text;

        while !remaining.is_empty() {
            let mut byte_end = 0;
            let mut chars_col = 0;

            for ch in remaining.chars() {
                let w = ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 0 });
                if col + chars_col + w > max_cols {
                    break;
                }
                byte_end += ch.len_utf8();
                chars_col += w;
            }

            if byte_end == 0 {
                // Single character wider than remaining space.
                if !current_line.is_empty() {
                    result.push(std::mem::take(&mut current_line));
                }
                let Some(ch) = remaining.chars().next() else {
                    break;
                };
                let ch_len = ch.len_utf8();
                current_line.push(RtSpan::styled(remaining[..ch_len].to_string(), style));
                col = ch.width().unwrap_or(if ch == '\t' { TAB_WIDTH } else { 1 });
                remaining = &remaining[ch_len..];
                continue;
            }

            let (chunk, rest) = remaining.split_at(byte_end);
            current_line.push(RtSpan::styled(chunk.to_string(), style));
            col += chars_col;
            remaining = rest;

            if col >= max_cols {
                result.push(std::mem::take(&mut current_line));
                col = 0;
            }
        }
    }

    if !current_line.is_empty() || result.is_empty() {
        result.push(current_line);
    }

    result
}

// -- Style helpers -----------------------------------------------------------

fn style_line_bg_for(kind: DiffLineType, diff_backgrounds: ResolvedDiffBackgrounds) -> Style {
    match kind {
        DiffLineType::Insert => diff_backgrounds
            .add
            .map_or_else(Style::default, |bg| Style::default().bg(bg)),
        DiffLineType::Delete => diff_backgrounds
            .del
            .map_or_else(Style::default, |bg| Style::default().bg(bg)),
        DiffLineType::Context => Style::default(),
    }
}

fn style_context() -> Style {
    Style::default()
}

fn add_line_bg(theme: DiffTheme, color_level: RichDiffColorLevel) -> Color {
    match (theme, color_level) {
        (DiffTheme::Dark, RichDiffColorLevel::TrueColor) => rgb_color(DARK_TC_ADD_LINE_BG_RGB),
        (DiffTheme::Dark, RichDiffColorLevel::Ansi256) => indexed_color(DARK_256_ADD_LINE_BG_IDX),
        (DiffTheme::Light, RichDiffColorLevel::TrueColor) => rgb_color(LIGHT_TC_ADD_LINE_BG_RGB),
        (DiffTheme::Light, RichDiffColorLevel::Ansi256) => indexed_color(LIGHT_256_ADD_LINE_BG_IDX),
    }
}

fn del_line_bg(theme: DiffTheme, color_level: RichDiffColorLevel) -> Color {
    match (theme, color_level) {
        (DiffTheme::Dark, RichDiffColorLevel::TrueColor) => rgb_color(DARK_TC_DEL_LINE_BG_RGB),
        (DiffTheme::Dark, RichDiffColorLevel::Ansi256) => indexed_color(DARK_256_DEL_LINE_BG_IDX),
        (DiffTheme::Light, RichDiffColorLevel::TrueColor) => rgb_color(LIGHT_TC_DEL_LINE_BG_RGB),
        (DiffTheme::Light, RichDiffColorLevel::Ansi256) => indexed_color(LIGHT_256_DEL_LINE_BG_IDX),
    }
}

fn light_gutter_fg(color_level: DiffColorLevel) -> Color {
    match color_level {
        DiffColorLevel::TrueColor => rgb_color(LIGHT_TC_GUTTER_FG_RGB),
        DiffColorLevel::Ansi256 => indexed_color(LIGHT_256_GUTTER_FG_IDX),
        DiffColorLevel::Ansi16 => Color::Black,
    }
}

fn light_add_num_bg(color_level: RichDiffColorLevel) -> Color {
    match color_level {
        RichDiffColorLevel::TrueColor => rgb_color(LIGHT_TC_ADD_NUM_BG_RGB),
        RichDiffColorLevel::Ansi256 => indexed_color(LIGHT_256_ADD_NUM_BG_IDX),
    }
}

fn light_del_num_bg(color_level: RichDiffColorLevel) -> Color {
    match color_level {
        RichDiffColorLevel::TrueColor => rgb_color(LIGHT_TC_DEL_NUM_BG_RGB),
        RichDiffColorLevel::Ansi256 => indexed_color(LIGHT_256_DEL_NUM_BG_IDX),
    }
}

fn style_gutter_for(kind: DiffLineType, theme: DiffTheme, color_level: DiffColorLevel) -> Style {
    match (
        theme,
        kind,
        RichDiffColorLevel::from_diff_color_level(color_level),
    ) {
        (DiffTheme::Light, DiffLineType::Insert, None) => {
            Style::default().fg(light_gutter_fg(color_level))
        }
        (DiffTheme::Light, DiffLineType::Delete, None) => {
            Style::default().fg(light_gutter_fg(color_level))
        }
        (DiffTheme::Light, DiffLineType::Insert, Some(level)) => Style::default()
            .fg(light_gutter_fg(color_level))
            .bg(light_add_num_bg(level)),
        (DiffTheme::Light, DiffLineType::Delete, Some(level)) => Style::default()
            .fg(light_gutter_fg(color_level))
            .bg(light_del_num_bg(level)),
        _ => style_gutter_dim(),
    }
}

fn style_sign_add(
    theme: DiffTheme,
    color_level: DiffColorLevel,
    diff_backgrounds: ResolvedDiffBackgrounds,
) -> Style {
    match theme {
        DiffTheme::Light => Style::default().fg(Color::Green),
        DiffTheme::Dark => style_add(theme, color_level, diff_backgrounds),
    }
}

fn style_sign_del(
    theme: DiffTheme,
    color_level: DiffColorLevel,
    diff_backgrounds: ResolvedDiffBackgrounds,
) -> Style {
    match theme {
        DiffTheme::Light => Style::default().fg(Color::Red),
        DiffTheme::Dark => style_del(theme, color_level, diff_backgrounds),
    }
}

fn style_add(
    theme: DiffTheme,
    color_level: DiffColorLevel,
    diff_backgrounds: ResolvedDiffBackgrounds,
) -> Style {
    match (theme, color_level, diff_backgrounds.add) {
        (_, DiffColorLevel::Ansi16, _) => Style::default().fg(Color::Green),
        (DiffTheme::Light, _, Some(bg)) => Style::default().bg(bg),
        (DiffTheme::Dark, _, Some(bg)) => Style::default().fg(Color::Green).bg(bg),
        (DiffTheme::Light, _, None) => Style::default(),
        (DiffTheme::Dark, _, None) => Style::default().fg(Color::Green),
    }
}

fn style_del(
    theme: DiffTheme,
    color_level: DiffColorLevel,
    diff_backgrounds: ResolvedDiffBackgrounds,
) -> Style {
    match (theme, color_level, diff_backgrounds.del) {
        (_, DiffColorLevel::Ansi16, _) => Style::default().fg(Color::Red),
        (DiffTheme::Light, _, Some(bg)) => Style::default().bg(bg),
        (DiffTheme::Dark, _, Some(bg)) => Style::default().fg(Color::Red).bg(bg),
        (DiffTheme::Light, _, None) => Style::default(),
        (DiffTheme::Dark, _, None) => Style::default().fg(Color::Red),
    }
}

fn style_gutter_dim() -> Style {
    Style::default().add_modifier(Modifier::DIM)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: create a unified diff from old/new content.
    fn make_diff(old: &str, new: &str) -> String {
        similar::TextDiff::from_lines(old, new)
            .unified_diff()
            .context_radius(3)
            .header("test.rs", "test.rs")
            .to_string()
    }

    #[test]
    fn render_simple_insert_diff() {
        let diff = make_diff("line1\nline2\n", "line1\ninserted\nline2\n");
        let lines = render_unified_diff(&diff, "test.rs", 80);

        assert!(!lines.is_empty());
        // Should contain an insert line with +
        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains('+'));
        assert!(text.contains("inserted"));
    }

    #[test]
    fn render_simple_delete_diff() {
        let diff = make_diff("line1\nremoved\nline2\n", "line1\nline2\n");
        let lines = render_unified_diff(&diff, "test.rs", 80);

        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        assert!(text.contains('-'));
        assert!(text.contains("removed"));
    }

    #[test]
    fn render_update_diff_has_context_lines() {
        let diff = make_diff(
            "fn main() {\n    println!(\"hello\");\n}\n",
            "fn main() {\n    println!(\"world\");\n}\n",
        );
        let lines = render_unified_diff(&diff, "test.rs", 80);

        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        // Context lines should be present.
        assert!(text.contains("fn main()"));
        // Insert and delete lines should be present.
        assert!(text.contains("hello"));
        assert!(text.contains("world"));
    }

    #[test]
    fn render_empty_diff_returns_empty() {
        let lines = render_unified_diff("", "test.rs", 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn render_nonsense_input_returns_empty() {
        // diffy parses nonsense as empty patch (0 hunks), so output is empty.
        let lines = render_unified_diff("not a valid diff", "test.rs", 80);
        assert!(lines.is_empty());
    }

    #[test]
    fn line_number_width_zero() {
        assert_eq!(line_number_width(0), 1);
    }

    #[test]
    fn line_number_width_single_digit() {
        assert_eq!(line_number_width(9), 1);
    }

    #[test]
    fn line_number_width_multi_digit() {
        assert_eq!(line_number_width(100), 3);
        assert_eq!(line_number_width(9999), 4);
    }

    #[test]
    fn wrap_styled_spans_no_wrap_needed() {
        let spans = vec![RtSpan::raw("short text")];
        let result = wrap_styled_spans(&spans, 80);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn wrap_styled_spans_forces_wrap() {
        let spans = vec![RtSpan::raw("a".repeat(100))];
        let result = wrap_styled_spans(&spans, 40);
        assert!(result.len() > 1);
    }

    #[test]
    fn diff_line_has_gutter_and_sign() {
        let ctx = DiffRenderStyleContext {
            theme: DiffTheme::Dark,
            color_level: DiffColorLevel::Ansi16,
            diff_backgrounds: ResolvedDiffBackgrounds::default(),
        };
        let p = DiffLineParams {
            line_number: 42,
            kind: DiffLineType::Insert,
            text: "hello",
            width: 80,
            line_number_width: 2,
            syntax_spans: None,
        };
        let lines = render_diff_line(&p, &ctx);

        assert_eq!(lines.len(), 1);
        let text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        // Should contain line number 42 and + sign.
        assert!(text.contains("42"));
        assert!(text.contains('+'));
        assert!(text.contains("hello"));
    }

    #[test]
    fn ansi16_uses_foreground_only() {
        let bg = fallback_diff_backgrounds(DiffTheme::Dark, DiffColorLevel::Ansi16);
        assert_eq!(bg.add, None);
        assert_eq!(bg.del, None);

        let add_style = style_add(DiffTheme::Dark, DiffColorLevel::Ansi16, bg);
        assert_eq!(add_style.fg, Some(Color::Green));
        assert_eq!(add_style.bg, None);

        let del_style = style_del(DiffTheme::Dark, DiffColorLevel::Ansi16, bg);
        assert_eq!(del_style.fg, Some(Color::Red));
        assert_eq!(del_style.bg, None);
    }

    #[test]
    fn truecolor_uses_background_tints() {
        let bg = fallback_diff_backgrounds(DiffTheme::Dark, DiffColorLevel::TrueColor);
        assert!(bg.add.is_some());
        assert!(bg.del.is_some());
    }

    #[test]
    fn hunk_separator_present_for_multi_hunk() {
        // Create a diff with two separate hunks by having changes far apart.
        let mut old = String::new();
        let mut new = String::new();
        for i in 0..30 {
            old.push_str(&format!("line{i}\n"));
            if i == 5 {
                new.push_str("changed_early\n");
            } else if i == 25 {
                new.push_str("changed_late\n");
            } else {
                new.push_str(&format!("line{i}\n"));
            }
        }
        let diff = make_diff(&old, &new);
        let lines = render_unified_diff(&diff, "test.txt", 80);

        let text: String = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .map(|s| s.content.as_ref())
            .collect();
        // Should contain hunk separator.
        assert!(text.contains('⋮'));
    }
}
