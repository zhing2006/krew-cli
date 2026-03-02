//! Minimal inline terminal with dynamic viewport height.

use std::io::{self, Write, stdout};

use crossterm::cursor::MoveTo;
use crossterm::queue;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType as CtClearType};
use ratatui::backend::{Backend, ClearType, CrosstermBackend};
use ratatui::buffer::{Buffer, Cell};
use ratatui::layout::{Position, Rect, Size};
use ratatui::text::Line;
use unicode_width::UnicodeWidthStr;

use super::Frame;
use super::ansi::{ResetScrollRegion, SetScrollRegion, write_spans};

/// Minimal inline terminal with dynamic viewport height.
pub struct Terminal {
    backend: CrosstermBackend<io::Stdout>,
    buffers: [Buffer; 2],
    current: usize,
    hidden_cursor: bool,
    /// Current viewport rectangle (public for read access).
    pub viewport_area: Rect,
    last_known_size: Size,
    last_known_cursor_pos: Position,
}

impl Drop for Terminal {
    fn drop(&mut self) {
        if self.hidden_cursor {
            let _ = self.backend.show_cursor();
        }
    }
}

impl Terminal {
    /// Create a new terminal. The viewport starts at the current cursor
    /// position with zero height; call [`ensure_viewport_height`] to
    /// reserve space.
    pub fn new() -> io::Result<Self> {
        let mut backend = CrosstermBackend::new(stdout());
        let size = backend.size()?;
        let pos = backend.get_cursor_position().unwrap_or(Position::ORIGIN);

        Ok(Self {
            backend,
            buffers: [Buffer::empty(Rect::ZERO), Buffer::empty(Rect::ZERO)],
            current: 0,
            hidden_cursor: false,
            viewport_area: Rect::new(0, pos.y, size.width, 0),
            last_known_size: size,
            last_known_cursor_pos: pos,
        })
    }

    /// Query the backend for the current terminal size.
    pub fn size(&self) -> io::Result<Size> {
        self.backend.size()
    }

    /// Mutable access to the backend (for raw crossterm commands).
    #[allow(dead_code)]
    pub fn backend_mut(&mut self) -> &mut CrosstermBackend<io::Stdout> {
        &mut self.backend
    }

    /// Resize both internal buffers and update the viewport area.
    pub fn set_viewport_area(&mut self, area: Rect) {
        self.buffers[self.current].resize(area);
        self.buffers[1 - self.current].resize(area);
        self.viewport_area = area;
    }

    /// Adjust the viewport to the given height, scrolling the screen up
    /// if necessary.
    pub fn ensure_viewport_height(&mut self, height: u16) -> io::Result<()> {
        let size = self.size()?;
        self.last_known_size = size;

        let mut area = self.viewport_area;
        // Cap viewport to half the terminal height so content inserted
        // above always has room in the scroll region.
        let max_height = (size.height / 2).max(4);
        area.height = height.min(max_height);
        area.width = size.width;

        // If the viewport extends beyond the screen bottom, scroll up.
        if area.bottom() > size.height {
            let delta = area.bottom() - size.height;
            self.scroll_up(delta)?;
            area.y = size.height - area.height;
        }

        if area != self.viewport_area {
            self.set_viewport_area(area);
            self.clear()?;
        }

        Ok(())
    }

    /// Draw a frame. The callback should render all widgets into the
    /// provided [`Frame`].
    pub fn draw<F>(&mut self, f: F) -> io::Result<()>
    where
        F: FnOnce(&mut Frame),
    {
        // Handle terminal resize (e.g. window dragged).
        let size = self.size()?;
        if size != self.last_known_size {
            let mut area = self.viewport_area;
            area.width = size.width;
            if area.bottom() > size.height {
                area.y = size.height.saturating_sub(area.height);
                area.height = area.height.min(size.height);
            }
            self.set_viewport_area(area);
            self.last_known_size = size;
        }

        let mut frame = Frame::new(self.viewport_area, &mut self.buffers[self.current]);
        f(&mut frame);
        let cursor_position = frame.cursor_position;

        // Hide cursor before flushing diffs to prevent visible cursor
        // jumping to each changed cell position during the update.
        if !self.hidden_cursor {
            self.backend.hide_cursor()?;
        }

        self.flush()?;

        match cursor_position {
            None => {
                self.hidden_cursor = true;
            }
            Some(pos) => {
                self.backend.set_cursor_position(pos)?;
                self.backend.show_cursor()?;
                self.hidden_cursor = false;
                self.last_known_cursor_pos = pos;
            }
        }

        self.swap_buffers();
        Backend::flush(&mut self.backend)?;
        Ok(())
    }

    /// Clear the viewport area on screen and reset the previous buffer.
    pub fn clear(&mut self) -> io::Result<()> {
        if self.viewport_area.is_empty() {
            return Ok(());
        }
        self.backend
            .set_cursor_position(self.viewport_area.as_position())?;
        self.backend.clear_region(ClearType::AfterCursor)?;
        self.buffers[1 - self.current].reset();
        Ok(())
    }

    // ── Insert above viewport ────────────────────────────────────────

    /// Insert rendered widget content above the viewport (scrolls into
    /// terminal scrollback). Uses a ratatui Buffer for arbitrary widget
    /// rendering (e.g. bordered boxes).
    pub fn insert_widget_above<F>(&mut self, height: u16, draw_fn: F) -> io::Result<()>
    where
        F: FnOnce(&mut Buffer),
    {
        if height == 0 {
            return Ok(());
        }

        let area = Rect::new(0, 0, self.viewport_area.width, height);
        let mut buffer = Buffer::empty(area);
        draw_fn(&mut buffer);
        let mut cells = buffer.content.as_slice();

        let mut drawn_height: i32 = self.viewport_area.top().into();
        let mut buffer_height: i32 = height.into();
        let viewport_height: i32 = self.viewport_area.height.into();
        let screen_height: i32 = self.last_known_size.height.into();

        // Draw in chunks, scrolling as needed.
        while buffer_height + viewport_height > screen_height {
            let to_draw = buffer_height.min(screen_height);
            let scroll = 0.max(drawn_height + to_draw - screen_height);
            self.scroll_up(scroll as u16)?;
            cells = self.draw_cells((drawn_height - scroll) as u16, to_draw as u16, cells)?;
            drawn_height += to_draw - scroll;
            buffer_height -= to_draw;
        }

        let scroll = 0.max(drawn_height + buffer_height + viewport_height - screen_height);
        self.scroll_up(scroll as u16)?;
        self.draw_cells((drawn_height - scroll) as u16, buffer_height as u16, cells)?;
        drawn_height += buffer_height - scroll;

        self.set_viewport_area(Rect {
            y: drawn_height as u16,
            ..self.viewport_area
        });
        self.clear()?;

        Ok(())
    }

    /// Insert styled lines above the viewport using scroll regions and
    /// direct ANSI output. Each line is terminated with a real `\r\n`, so
    /// the terminal emulator preserves newlines when the user copies text.
    pub fn insert_lines_above(&mut self, lines: Vec<Line<'static>>) -> io::Result<()> {
        if lines.is_empty() {
            return Ok(());
        }

        let screen_size = self.size()?;
        let mut area = self.viewport_area;
        let last_cursor_pos = self.last_known_cursor_pos;
        let mut out = stdout();
        let line_count = lines.len() as u16;

        let cursor_top = if area.bottom() < screen_size.height {
            // Viewport is not at the bottom — scroll region downward to
            // make room above the viewport.
            let scroll_amount = line_count.min(screen_size.height - area.bottom());
            let top_1based = area.top() + 1;
            queue!(out, SetScrollRegion(top_1based..screen_size.height))?;
            queue!(out, MoveTo(0, area.top()))?;
            for _ in 0..scroll_amount {
                // Reverse Index (RI): scroll region content down.
                queue!(out, Print("\x1bM"))?;
            }
            queue!(out, ResetScrollRegion)?;

            let cursor_top = area.top().saturating_sub(1);
            area.y += scroll_amount;
            cursor_top
        } else {
            area.top().saturating_sub(1)
        };

        // Limit scroll region to the area above the viewport so the
        // viewport itself is not pushed off screen.
        queue!(out, SetScrollRegion(1..area.top()))?;
        queue!(out, MoveTo(0, cursor_top))?;

        for line in &lines {
            queue!(out, Print("\r\n"))?;
            queue!(out, Clear(CtClearType::UntilNewLine))?;
            write_spans(&mut out, line)?;
        }

        queue!(out, ResetScrollRegion)?;
        queue!(out, MoveTo(last_cursor_pos.x, last_cursor_pos.y))?;
        out.flush()?;

        if area != self.viewport_area {
            self.set_viewport_area(area);
        }

        Ok(())
    }

    // ── Private helpers ──────────────────────────────────────────────

    /// Diff previous and current buffers, writing only the changes.
    fn flush(&mut self) -> io::Result<()> {
        let previous = &self.buffers[1 - self.current];
        let current = &self.buffers[self.current];
        let updates = previous.diff(current);
        if let Some((col, row, _)) = updates.last() {
            self.last_known_cursor_pos = Position { x: *col, y: *row };
        }
        self.backend.draw(updates.into_iter())?;
        Ok(())
    }

    /// Clear the inactive buffer and swap.
    fn swap_buffers(&mut self) {
        self.buffers[1 - self.current].reset();
        self.current = 1 - self.current;
    }

    /// Scroll the entire screen up by `n` lines.
    fn scroll_up(&mut self, n: u16) -> io::Result<()> {
        if n > 0 {
            self.backend.set_cursor_position(Position::new(
                0,
                self.last_known_size.height.saturating_sub(1),
            ))?;
            self.backend.append_lines(n)?;
        }
        Ok(())
    }

    /// Draw cells at a given y-offset, returning the unused tail.
    ///
    /// Wide characters (CJK, emoji) occupy multiple buffer cells but only the
    /// first cell contains the actual symbol. Continuation cells must be
    /// skipped to avoid overwriting the trailing columns of a wide glyph.
    fn draw_cells<'a>(
        &mut self,
        y_offset: u16,
        lines: u16,
        cells: &'a [Cell],
    ) -> io::Result<&'a [Cell]> {
        let width = self.viewport_area.width as usize;
        let count = width * lines as usize;
        let (to_draw, rest) = cells.split_at(count.min(cells.len()));
        if lines > 0 && !to_draw.is_empty() {
            let mut to_skip: usize = 0;
            let items: Vec<_> = to_draw
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    if to_skip > 0 {
                        to_skip -= 1;
                        return None;
                    }
                    to_skip = c.symbol().width().saturating_sub(1);
                    Some(((i % width) as u16, y_offset + (i / width) as u16, c))
                })
                .collect();
            self.backend.draw(items.into_iter())?;
            Backend::flush(&mut self.backend)?;
        }
        Ok(rest)
    }
}
