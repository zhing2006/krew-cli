//! Terminal color capability detection.
//!
//! Uses env var heuristics instead of `supports-color` crate.

use ratatui::style::Color;
use std::sync::OnceLock;

/// Terminal stdout color capability level.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StdoutColorLevel {
    TrueColor,
    Ansi256,
    Ansi16,
}

/// Detect the terminal's color support level from environment variables.
///
/// Checks `COLORTERM`, `TERM`, and `WT_SESSION` to determine capability.
pub fn stdout_color_level() -> StdoutColorLevel {
    static LEVEL: OnceLock<StdoutColorLevel> = OnceLock::new();
    *LEVEL.get_or_init(detect_color_level)
}

fn detect_color_level() -> StdoutColorLevel {
    // COLORTERM=truecolor or COLORTERM=24bit → TrueColor
    if let Ok(ct) = std::env::var("COLORTERM") {
        let ct = ct.to_lowercase();
        if ct == "truecolor" || ct == "24bit" {
            return StdoutColorLevel::TrueColor;
        }
    }

    // Windows Terminal always supports TrueColor
    if std::env::var_os("WT_SESSION").is_some() {
        return StdoutColorLevel::TrueColor;
    }

    // TERM contains "256color" → Ansi256
    if let Ok(term) = std::env::var("TERM") {
        if term.contains("256color") {
            return StdoutColorLevel::Ansi256;
        }
        if term.contains("color") || term.contains("xterm") || term.contains("screen") {
            return StdoutColorLevel::Ansi16;
        }
    }

    // Windows without WT_SESSION — most modern terminals support TrueColor
    if cfg!(windows) {
        StdoutColorLevel::TrueColor
    } else {
        StdoutColorLevel::Ansi16
    }
}

/// Construct a ratatui `Color::Rgb` from an (r, g, b) tuple.
pub fn rgb_color((r, g, b): (u8, u8, u8)) -> Color {
    Color::Rgb(r, g, b)
}

/// Construct a ratatui `Color::Indexed` from an xterm-256 index.
pub fn indexed_color(index: u8) -> Color {
    Color::Indexed(index)
}

/// Query the terminal's default background color.
///
/// Currently returns None on all platforms (defaults to dark theme).
/// Official crossterm does not expose an OSC color query API; the Codex
/// project uses a custom fork for this. We may revisit once upstream
/// crossterm gains native support.
pub fn default_bg() -> Option<(u8, u8, u8)> {
    None
}
