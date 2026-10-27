//! Window: frame rect + client surface + chrome metrics.

use crate::geom::{Point, Rect};
use crate::surface::Surface;
use alloc::string::String;

/// Title bar height (px).
pub const TITLE_H: i32 = 26;
/// Border width (px).
pub const BORDER: i32 = 1;
/// Close button box size (px), placed inside the title bar.
pub const CLOSE: i32 = 18;

/// Regions of a window frame that a click can land on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit {
    Close,
    Title,
    Client,
}

pub struct Window {
    /// Stable identity — window order changes with focus, ids don't.
    pub id: usize,
    pub title: String,
    /// Outer frame, including title bar + border.
    pub frame: Rect,
    /// Client content, `frame.w` x `frame.h - TITLE_H - 2*BORDER`.
    pub content: Surface,
    pub closeable: bool,
    /// Incremented by the app when it repaints `content`.
    pub content_revision: u64,
}

impl Window {
    /// `w`/`h` describe the client area; chrome is added around it.
    pub fn new(title: &str, x: i32, y: i32, w: i32, h: i32) -> Self {
        Self {
            id: usize::MAX,
            title: String::from(title),
            frame: Rect::new(x, y, w + 2 * BORDER, h + TITLE_H + 2 * BORDER),
            content: Surface::new(w as usize, h as usize, crate::surface::TERMINAL_BG),
            closeable: true,
            content_revision: 0,
        }
    }

    /// Screen rect of the client area.
    pub fn client_rect(&self) -> Rect {
        Rect::new(
            self.frame.x + BORDER,
            self.frame.y + TITLE_H + BORDER,
            self.frame.w - 2 * BORDER,
            self.frame.h - TITLE_H - 2 * BORDER,
        )
    }

    /// Close button rect in screen space.
    pub fn close_rect(&self) -> Rect {
        Rect::new(
            self.frame.x + self.frame.w - BORDER - CLOSE - 3,
            self.frame.y + (TITLE_H - CLOSE) / 2,
            CLOSE,
            CLOSE,
        )
    }

    /// Title bar rect (between borders, below top border).
    pub fn title_rect(&self) -> Rect {
        Rect::new(
            self.frame.x + BORDER,
            self.frame.y + BORDER,
            self.frame.w - 2 * BORDER,
            TITLE_H,
        )
    }

    /// Classify a screen-space point.
    pub fn hit(&self, p: Point) -> Option<Hit> {
        if !self.frame.contains(p) {
            return None;
        }
        if self.closeable && self.close_rect().contains(p) {
            Some(Hit::Close)
        } else if self.title_rect().contains(p) {
            Some(Hit::Title)
        } else {
            Some(Hit::Client)
        }
    }
}
