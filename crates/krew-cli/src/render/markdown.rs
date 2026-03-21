//! Markdown rendering to ratatui styled Lines.
//!
//! Uses pulldown_cmark for CommonMark parsing and syntect for code highlighting.

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use super::highlight::highlight_code_to_lines;

/// Render a Markdown string into a list of ratatui Lines.
pub fn render_markdown(text: &str) -> Vec<Line<'static>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = Parser::new_ext(text, options);

    let mut renderer = MarkdownRenderer::new();
    renderer.process(parser);
    renderer.finish()
}

struct MarkdownRenderer {
    /// Accumulated output lines.
    lines: Vec<Line<'static>>,
    /// Current line spans being built.
    current_spans: Vec<Span<'static>>,
    /// Style stack for nested inline formatting.
    style_stack: Vec<Style>,
    /// Whether we are inside a code block.
    in_code_block: bool,
    /// Language for the current code block.
    code_block_lang: Option<String>,
    /// Accumulated code block content.
    code_block_buf: String,
    /// List nesting state: (is_ordered, item_number).
    list_stack: Vec<ListState>,
    /// Current indent prefix for list items.
    list_indent: String,
    /// Whether we just emitted a list item marker (suppresses leading newline).
    after_list_item_start: bool,
    /// Whether we are inside a blockquote.
    in_blockquote: bool,
    /// Whether the next block-level element should be preceded by a blank line.
    needs_newline: bool,
}

#[derive(Clone)]
struct ListState {
    ordered: bool,
    next_number: u64,
}

impl MarkdownRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_spans: Vec::new(),
            style_stack: vec![Style::default()],
            in_code_block: false,
            code_block_lang: None,
            code_block_buf: String::new(),
            list_stack: Vec::new(),
            list_indent: String::new(),
            after_list_item_start: false,
            in_blockquote: false,
            needs_newline: false,
        }
    }

    fn current_style(&self) -> Style {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, modifier: fn(Style) -> Style) {
        let new_style = modifier(self.current_style());
        self.style_stack.push(new_style);
    }

    fn pop_style(&mut self) {
        if self.style_stack.len() > 1 {
            self.style_stack.pop();
        }
    }

    fn flush_line(&mut self) {
        if !self.current_spans.is_empty() {
            let spans = std::mem::take(&mut self.current_spans);
            self.lines.push(Line::from(spans));
        }
    }

    fn emit_text(&mut self, text: &str) {
        if self.in_code_block {
            self.code_block_buf.push_str(text);
            return;
        }

        let style = self.current_style();

        // Handle text that contains newlines.
        let mut parts = text.split('\n');
        if let Some(first) = parts.next() {
            if !first.is_empty() {
                self.current_spans
                    .push(Span::styled(first.to_string(), style));
            }

            for part in parts {
                self.flush_line();
                if !part.is_empty() {
                    self.current_spans
                        .push(Span::styled(part.to_string(), style));
                }
            }
        }
    }

    fn process<'a>(&mut self, parser: Parser<'a>) {
        for event in parser {
            match event {
                Event::Start(tag) => self.handle_start(tag),
                Event::End(tag) => self.handle_end(tag),
                Event::Text(text) => self.emit_text(&text),
                Event::Code(code) => {
                    let style = self.current_style().fg(Color::Cyan);
                    self.current_spans
                        .push(Span::styled(format!("`{code}`"), style));
                }
                Event::SoftBreak => {
                    // Treat soft breaks as hard line breaks. CommonMark spec
                    // renders single \n as a space, but LLM output uses \n to
                    // mean "new line" (e.g. poetry, lists without markers).
                    self.flush_line();
                }
                Event::HardBreak => {
                    self.flush_line();
                }
                Event::Rule => {
                    self.flush_line();
                    if self.needs_newline {
                        self.lines.push(Line::default());
                    }
                    self.lines.push(Line::from(Span::styled(
                        "───────────────────────────────────────".to_string(),
                        Style::new().fg(Color::DarkGray),
                    )));
                    self.needs_newline = true;
                }
                _ => {}
            }
        }
    }

    fn handle_start(&mut self, tag: Tag) {
        match tag {
            Tag::Paragraph => {
                // Insert blank separator between paragraphs (but not after list item start).
                if self.needs_newline && !self.after_list_item_start {
                    self.flush_line();
                    self.lines.push(Line::default());
                }
                self.after_list_item_start = false;
                self.needs_newline = false;
            }
            Tag::Heading { level, .. } => {
                self.flush_line();
                if self.needs_newline {
                    self.lines.push(Line::default());
                }
                self.needs_newline = false;
                let style = match level {
                    HeadingLevel::H1 => {
                        Style::new().add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    }
                    HeadingLevel::H2 => Style::new().add_modifier(Modifier::BOLD),
                    HeadingLevel::H3 => {
                        Style::new().add_modifier(Modifier::BOLD | Modifier::ITALIC)
                    }
                    _ => Style::new().add_modifier(Modifier::BOLD),
                };
                self.style_stack.push(style);
            }
            Tag::BlockQuote(_) => {
                self.flush_line();
                if self.needs_newline {
                    self.lines.push(Line::default());
                }
                self.needs_newline = false;
                self.in_blockquote = true;
                self.push_style(|s| s.fg(Color::Green));
            }
            Tag::CodeBlock(kind) => {
                self.flush_line();
                if self.needs_newline {
                    self.lines.push(Line::default());
                }
                self.needs_newline = false;
                self.in_code_block = true;
                self.code_block_buf.clear();
                self.code_block_lang = match kind {
                    CodeBlockKind::Fenced(lang) => {
                        let lang = lang.to_string();
                        if lang.is_empty() { None } else { Some(lang) }
                    }
                    CodeBlockKind::Indented => None,
                };
            }
            Tag::List(start) => {
                if self.list_stack.is_empty() {
                    self.flush_line();
                    if self.needs_newline {
                        self.lines.push(Line::default());
                    }
                }
                self.needs_newline = false;
                self.list_stack.push(ListState {
                    ordered: start.is_some(),
                    next_number: start.unwrap_or(1),
                });
                self.update_list_indent();
            }
            Tag::Item => {
                self.flush_line();
                self.needs_newline = false;
                let marker = if let Some(state) = self.list_stack.last_mut() {
                    if state.ordered {
                        let num = state.next_number;
                        state.next_number += 1;
                        format!("{}{num}. ", self.list_indent)
                    } else {
                        format!("{}• ", self.list_indent)
                    }
                } else {
                    "• ".to_string()
                };
                self.current_spans
                    .push(Span::styled(marker, self.current_style()));
                self.after_list_item_start = true;
            }
            Tag::Emphasis => {
                self.push_style(|s| s.add_modifier(Modifier::ITALIC));
            }
            Tag::Strong => {
                self.push_style(|s| s.add_modifier(Modifier::BOLD));
            }
            Tag::Strikethrough => {
                self.push_style(|s| s.add_modifier(Modifier::CROSSED_OUT));
            }
            Tag::Link { .. } => {
                self.push_style(|s| s.fg(Color::Cyan).add_modifier(Modifier::UNDERLINED));
            }
            _ => {}
        }
    }

    fn handle_end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.flush_line();
                self.needs_newline = true;
            }
            TagEnd::Heading(_) => {
                self.flush_line();
                self.pop_style();
                self.needs_newline = true;
            }
            TagEnd::BlockQuote(_) => {
                self.flush_line();
                self.in_blockquote = false;
                self.pop_style();
                self.needs_newline = true;
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                let code = std::mem::take(&mut self.code_block_buf);
                let lang = self.code_block_lang.take();
                // Remove trailing newline from code block content.
                let code = code.trim_end_matches('\n');
                let highlighted = highlight_code_to_lines(code, lang.as_deref());
                self.lines.extend(highlighted);
                self.needs_newline = true;
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                self.update_list_indent();
                if self.list_stack.is_empty() {
                    self.flush_line();
                }
                self.needs_newline = true;
            }
            TagEnd::Item => {
                self.flush_line();
                self.after_list_item_start = false;
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                self.pop_style();
            }
            _ => {}
        }
    }

    fn update_list_indent(&mut self) {
        let depth = self.list_stack.len();
        self.list_indent = if depth > 1 {
            "  ".repeat(depth - 1)
        } else {
            String::new()
        };
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_line();
        self.lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let lines = render_markdown("hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].content, "hello world");
    }

    #[test]
    fn test_bold() {
        let lines = render_markdown("**bold text**");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.content, "bold text");
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
    }

    #[test]
    fn test_italic() {
        let lines = render_markdown("*italic*");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.content, "italic");
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_inline_code() {
        let lines = render_markdown("use `Vec<T>`");
        assert_eq!(lines.len(), 1);
        // Find the code span.
        let code_span = lines[0]
            .spans
            .iter()
            .find(|s| s.content.contains("Vec<T>"))
            .expect("should have code span");
        assert_eq!(code_span.style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_heading_h1() {
        let lines = render_markdown("# Title");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_heading_h2() {
        let lines = render_markdown("## Subtitle");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(!span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_heading_h3() {
        let lines = render_markdown("### Section");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::BOLD));
        assert!(span.style.add_modifier.contains(Modifier::ITALIC));
    }

    #[test]
    fn test_unordered_list() {
        let lines = render_markdown("- item 1\n- item 2");
        assert!(lines.len() >= 2);
        let first_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_text.contains("•"));
        assert!(first_text.contains("item 1"));
    }

    #[test]
    fn test_ordered_list() {
        let lines = render_markdown("1. first\n2. second");
        assert!(lines.len() >= 2);
        let first_text: String = lines[0].spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(first_text.contains("1."));
        assert!(first_text.contains("first"));
    }

    #[test]
    fn test_code_block_with_language() {
        let md = "```rust\nfn main() {}\n```";
        let lines = render_markdown(md);
        assert!(!lines.is_empty());
        // Should have syntax-highlighted spans (not plain cyan).
    }

    #[test]
    fn test_code_block_without_language() {
        let md = "```\nsome code\n```";
        let lines = render_markdown(md);
        assert!(!lines.is_empty());
        // Should fallback to plain cyan.
        assert_eq!(lines[0].spans[0].style.fg, Some(Color::Cyan));
    }

    #[test]
    fn test_blockquote() {
        let lines = render_markdown("> quoted text");
        assert!(!lines.is_empty());
        let span = &lines[0].spans[0];
        assert_eq!(span.style.fg, Some(Color::Green));
    }

    #[test]
    fn test_strikethrough() {
        let lines = render_markdown("~~deleted~~");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert!(span.style.add_modifier.contains(Modifier::CROSSED_OUT));
    }

    #[test]
    fn test_link() {
        let lines = render_markdown("[click](https://example.com)");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.style.fg, Some(Color::Cyan));
        assert!(span.style.add_modifier.contains(Modifier::UNDERLINED));
    }

    #[test]
    fn test_paragraph_spacing() {
        let lines = render_markdown("para 1\n\npara 2\n\npara 3");
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        // Expect: ["para 1", "", "para 2", "", "para 3"]
        assert_eq!(texts, vec!["para 1", "", "para 2", "", "para 3"]);
    }

    #[test]
    fn test_tight_list_no_blanks() {
        let lines = render_markdown("- item 1\n- item 2\n- item 3");
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        // Tight list items should NOT have blank lines between them.
        assert_eq!(texts.len(), 3);
        assert!(texts[0].contains("item 1"));
        assert!(texts[1].contains("item 2"));
        assert!(texts[2].contains("item 3"));
    }

    #[test]
    fn test_paragraph_then_list_spacing() {
        let lines = render_markdown("Some text.\n\n- item 1\n- item 2");
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        // Blank separator between paragraph and list.
        assert_eq!(texts[0], "Some text.");
        assert_eq!(texts[1], "");
        assert!(texts[2].contains("item 1"));
        assert!(texts[3].contains("item 2"));
    }

    #[test]
    fn test_list_then_paragraph_has_blank() {
        // Blank line between list end and following paragraph.
        let lines = render_markdown("- item 1\n- item 2\n\nSome text.");
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        assert!(texts[0].contains("item 1"));
        assert!(texts[1].contains("item 2"));
        assert_eq!(texts[2], "");
        assert_eq!(texts[3], "Some text.");
    }

    #[test]
    fn test_heading_list_heading_spacing() {
        let md = "**Section 1**\n\n* item a\n* item b\n\n**Section 2**\n\n* item c";
        let lines = render_markdown(md);
        let texts: Vec<String> = lines
            .iter()
            .map(|l| l.spans.iter().map(|s| s.content.as_ref()).collect())
            .collect();
        // heading, blank, item, item, blank, heading, blank, item
        assert_eq!(texts.len(), 8);
        assert!(texts[0].contains("Section 1"));
        assert_eq!(texts[1], ""); // blank before list
        assert!(texts[2].contains("item a"));
        assert!(texts[3].contains("item b"));
        assert_eq!(texts[4], ""); // blank after list
        assert!(texts[5].contains("Section 2"));
        assert_eq!(texts[6], ""); // blank before list
        assert!(texts[7].contains("item c"));
    }
}
