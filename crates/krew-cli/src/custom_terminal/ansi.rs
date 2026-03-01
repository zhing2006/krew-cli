//! ANSI escape sequence helpers for direct terminal output.
//!
//! Provides scroll region commands and styled span writing used by
//! `Terminal::insert_lines_above`.

use std::fmt;
use std::io::{self, Write};

use crossterm::queue;
use crossterm::style::{
    Attribute, Colors, Print, SetAttribute, SetBackgroundColor, SetColors, SetForegroundColor,
};
use ratatui::style::{Color, Modifier};
use ratatui::text::Line;

// ── Scroll region commands ───────────────────────────────────────────────

/// DECSTBM — Set Top and Bottom Margins (scroll region).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SetScrollRegion(pub std::ops::Range<u16>);

impl crossterm::Command for SetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[{};{}r", self.0.start, self.0.end)
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        // ANSI-only; Windows Terminal supports this.
        Err(io::Error::other("SetScrollRegion: use ANSI"))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

/// Reset scroll region to the full screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ResetScrollRegion;

impl crossterm::Command for ResetScrollRegion {
    fn write_ansi(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "\x1b[r")
    }

    #[cfg(windows)]
    fn execute_winapi(&self) -> io::Result<()> {
        Err(io::Error::other("ResetScrollRegion: use ANSI"))
    }

    #[cfg(windows)]
    fn is_ansi_code_supported(&self) -> bool {
        true
    }
}

// ── Color conversion ─────────────────────────────────────────────────────

/// Convert ratatui `Color` to crossterm `Color`.
fn to_ct_color(c: Color) -> crossterm::style::Color {
    match c {
        Color::Reset => crossterm::style::Color::Reset,
        Color::Black => crossterm::style::Color::Black,
        Color::Red => crossterm::style::Color::DarkRed,
        Color::Green => crossterm::style::Color::DarkGreen,
        Color::Yellow => crossterm::style::Color::DarkYellow,
        Color::Blue => crossterm::style::Color::DarkBlue,
        Color::Magenta => crossterm::style::Color::DarkMagenta,
        Color::Cyan => crossterm::style::Color::DarkCyan,
        Color::Gray => crossterm::style::Color::Grey,
        Color::DarkGray => crossterm::style::Color::DarkGrey,
        Color::LightRed => crossterm::style::Color::Red,
        Color::LightGreen => crossterm::style::Color::Green,
        Color::LightYellow => crossterm::style::Color::Yellow,
        Color::LightBlue => crossterm::style::Color::Blue,
        Color::LightMagenta => crossterm::style::Color::Magenta,
        Color::LightCyan => crossterm::style::Color::Cyan,
        Color::White => crossterm::style::Color::White,
        Color::Rgb(r, g, b) => crossterm::style::Color::Rgb { r, g, b },
        Color::Indexed(i) => crossterm::style::Color::AnsiValue(i),
    }
}

// ── Span writing ─────────────────────────────────────────────────────────

/// Write ratatui `Span`s as ANSI-styled text to a writer.
pub(super) fn write_spans(writer: &mut impl Write, line: &Line<'_>) -> io::Result<()> {
    let mut fg = Color::Reset;
    let mut bg = Color::Reset;
    let mut cur_mod = Modifier::empty();

    // Apply line-level style.
    let line_fg = line.style.fg.unwrap_or(Color::Reset);
    let line_bg = line.style.bg.unwrap_or(Color::Reset);

    for span in &line.spans {
        let span_fg = span.style.fg.unwrap_or(line_fg);
        let span_bg = span.style.bg.unwrap_or(line_bg);

        let mut m = Modifier::empty();
        m.insert(span.style.add_modifier);
        m.remove(span.style.sub_modifier);

        if m != cur_mod {
            // Reset and re-apply for simplicity.
            queue!(writer, SetAttribute(Attribute::Reset))?;
            if m.contains(Modifier::BOLD) {
                queue!(writer, SetAttribute(Attribute::Bold))?;
            }
            if m.contains(Modifier::DIM) {
                queue!(writer, SetAttribute(Attribute::Dim))?;
            }
            if m.contains(Modifier::ITALIC) {
                queue!(writer, SetAttribute(Attribute::Italic))?;
            }
            if m.contains(Modifier::UNDERLINED) {
                queue!(writer, SetAttribute(Attribute::Underlined))?;
            }
            if m.contains(Modifier::REVERSED) {
                queue!(writer, SetAttribute(Attribute::Reverse))?;
            }
            cur_mod = m;
            // Force color re-emit after reset.
            fg = Color::Reset;
            bg = Color::Reset;
        }

        if span_fg != fg || span_bg != bg {
            queue!(
                writer,
                SetColors(Colors::new(to_ct_color(span_fg), to_ct_color(span_bg)))
            )?;
            fg = span_fg;
            bg = span_bg;
        }

        queue!(writer, Print(&*span.content))?;
    }

    queue!(
        writer,
        SetForegroundColor(crossterm::style::Color::Reset),
        SetBackgroundColor(crossterm::style::Color::Reset),
        SetAttribute(Attribute::Reset),
    )?;

    Ok(())
}
