//! Custom inline terminal with dynamic viewport height.
//!
//! ratatui's built-in Terminal keeps `viewport_area` private and fixes the
//! inline height at creation time. This module exposes `set_viewport_area`
//! so the viewport can grow/shrink dynamically as the input textarea
//! changes height.

mod ansi;
mod frame;
mod terminal;

pub use frame::Frame;
pub use terminal::Terminal;
