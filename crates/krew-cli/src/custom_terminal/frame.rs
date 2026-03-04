//! Render frame backed by a [`ratatui::buffer::Buffer`].

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};

/// Render frame backed by a [`Buffer`].
pub struct Frame<'a> {
    pub(super) cursor_position: Option<Position>,
    viewport_area: Rect,
    buffer: &'a mut Buffer,
}

impl<'a> Frame<'a> {
    /// Create a new frame for the given viewport area and buffer.
    pub(super) fn new(viewport_area: Rect, buffer: &'a mut Buffer) -> Self {
        Self {
            cursor_position: None,
            viewport_area,
            buffer,
        }
    }

    /// Area available for rendering.
    pub const fn area(&self) -> Rect {
        self.viewport_area
    }

    /// Render any [`ratatui::widgets::Widget`] into the buffer.
    pub fn render_widget<W: ratatui::widgets::Widget>(&mut self, widget: W, area: Rect) {
        widget.render(area, self.buffer);
    }

    /// Request the cursor to be shown at `position` after this frame.
    pub fn set_cursor_position<P: Into<Position>>(&mut self, position: P) {
        self.cursor_position = Some(position.into());
    }

    /// Direct access to the underlying buffer.
    pub fn buffer_mut(&mut self) -> &mut Buffer {
        self.buffer
    }
}
