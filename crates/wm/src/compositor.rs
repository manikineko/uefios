//! Compositor: desktop background, windows, taskbar, cursor — composited into a
//! screen-sized back buffer, then flushed to the framebuffer per damage rect.

use crate::cursor::{draw_cursor, CURSOR_H, CURSOR_W};
use crate::font::{draw_text, CELL_H};
use crate::geom::{Point, Rect};
use crate::surface::{rgb, Px, Surface};
use crate::surface::{BORDER as BORDER_C, CLOSE_BTN, DESKTOP, TERMINAL_FG, TITLE_ACTIVE, TITLE_INACTIVE, WHITE};
use crate::window::{Hit, Window, BORDER, CLOSE, TITLE_H};
use alloc::string::String;
use alloc::vec::Vec;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

const TASKBAR_H: i32 = 32;
const MAX_DAMAGE: usize = 24;

/// How a `BltPixel {r,g,b}` maps to the mode's 32-bit memory layout.
#[derive(Clone, Copy)]
enum Fmt {
    /// Bytes R,G,B,X in memory -> u32 LE = r | g<<8 | b<<16
    Rgb,
    /// Bytes B,G,R,X in memory -> u32 LE = b | g<<8 | r<<16
    Bgr,
    /// Channel masks from the mode info: (r_shift, r_mask, g, b, reserved_unused)
    Mask { rs: u32, gs: u32, bs: u32 },
    /// No direct framebuffer — must go through `Blt`.
    BltOnly,
}

impl Fmt {
    fn pack(self, p: Px) -> u32 {
        match self {
            Fmt::Rgb => p.red as u32 | (p.green as u32) << 8 | (p.blue as u32) << 16,
            Fmt::Bgr => p.blue as u32 | (p.green as u32) << 8 | (p.red as u32) << 16,
            Fmt::Mask { rs, gs, bs } => {
                (p.red as u32) << rs | (p.green as u32) << gs | (p.blue as u32) << bs
            }
            Fmt::BltOnly => 0,
        }
    }
}

fn mask_shift(m: u32) -> u32 {
    m.trailing_zeros()
}

/// What a mouse press produced — returned so `os` can route it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickOutcome {
    Desktop,
    Focus(usize),
    Closed(usize),
    ClientClick(usize, Point),
}

pub struct Compositor {
    pub screen: Rect,
    back: Surface,
    pub windows: Vec<Window>,
    dirty: Vec<Rect>,
    cursor: Point,
    focused: Option<usize>,
    dragging: Option<(usize, Point)>,
    pending_close: Option<usize>,
    taskbar: Rect,
    fb: *mut u8,
    fb_stride: usize,
    fmt: Fmt,
    /// Free-text shown right-aligned in the taskbar (status/debug).
    pub status: String,
}

impl Compositor {
    /// Takes the screen geometry + framebuffer from the active GOP mode.
    pub fn new(gop: &mut GraphicsOutput) -> Self {
        let info = gop.current_mode_info();
        let (w, h) = info.resolution();
        let stride = info.stride();
        let fmt = match info.pixel_format() {
            PixelFormat::Rgb => Fmt::Rgb,
            PixelFormat::Bgr => Fmt::Bgr,
            PixelFormat::Bitmask => {
                let bm = info.pixel_bitmask().unwrap_or_default();
                Fmt::Mask {
                    rs: mask_shift(bm.red),
                    gs: mask_shift(bm.green),
                    bs: mask_shift(bm.blue),
                }
            }
            PixelFormat::BltOnly => Fmt::BltOnly,
        };
        let fb = gop.frame_buffer().as_mut_ptr();
        let screen = Rect::new(0, 0, w as i32, h as i32);
        Self {
            screen,
            back: Surface::new(w, h, DESKTOP),
            windows: Vec::new(),
            dirty: alloc::vec![screen],
            cursor: Point { x: w as i32 / 2, y: h as i32 / 2 },
            focused: None,
            dragging: None,
            pending_close: None,
            taskbar: Rect::new(0, h as i32 - TASKBAR_H, w as i32, TASKBAR_H),
            fb,
            fb_stride: stride,
            fmt,
            status: String::new(),
        }
    }

    /// Framebuffer diagnostics: (base addr, stride px, fmt tag 0=Rgb 1=Bgr 2=Mask 3=BltOnly).
    pub fn fb_info(&self) -> (usize, usize, usize) {
        let tag = match self.fmt {
            Fmt::Rgb => 0,
            Fmt::Bgr => 1,
            Fmt::Mask { .. } => 2,
            Fmt::BltOnly => 3,
        };
        (self.fb as usize, self.fb_stride, tag)
    }

    pub fn set_status(&mut self, s: String) {
        if s != self.status {
            self.status = s;
            self.damage(self.taskbar);
        }
    }

    /// Mark a screen rect for repainting.
    pub fn damage(&mut self, r: Rect) {
        let r = r.intersect(self.screen);
        if r.is_empty() {
            return;
        }
        for d in self.dirty.iter_mut() {
            if !d.intersect(r).is_empty() {
                *d = d.union(r);
                return;
            }
        }
        if self.dirty.len() >= MAX_DAMAGE {
            self.dirty = alloc::vec![self.dirty.iter().fold(r, |a, b| a.union(*b))];
        } else {
            self.dirty.push(r);
        }
    }

    pub fn add_window(&mut self, win: Window) {
        let idx = self.windows.len();
        self.damage(win.frame);
        self.windows.push(win);
        self.focus(idx);
    }

    /// Take ownership of a window scheduled for closing (if any).
    pub fn take_pending_close(&mut self) -> Option<usize> {
        self.pending_close.take()
    }

    pub fn focused(&self) -> Option<usize> {
        self.focused
    }

    pub fn focus(&mut self, idx: usize) {
        if idx >= self.windows.len() {
            return;
        }
        if self.focused == Some(idx) && idx == self.windows.len() - 1 {
            return;
        }
        let win = self.windows.remove(idx);
        self.windows.push(win);
        self.focused = Some(self.windows.len() - 1);
        // Z-order changed: repaint the whole screen (simple + correct).
        self.damage(self.screen);
    }

    fn close(&mut self, idx: usize) {
        self.windows.remove(idx);
        self.damage(self.screen);
        self.focused = if self.windows.is_empty() {
            None
        } else {
            Some(self.windows.len() - 1)
        };
        self.pending_close = Some(idx);
    }

    /// Update cursor position (screen coords), handling window drags.
    pub fn mouse_move(&mut self, p: Point) {
        if p == self.cursor {
            return;
        }
        let old = Rect::new(self.cursor.x, self.cursor.y, CURSOR_W as i32, CURSOR_H as i32);
        self.cursor = Point {
            x: p.x.clamp(0, self.screen.w - 1),
            y: p.y.clamp(0, self.screen.h - 1),
        };
        let new = Rect::new(self.cursor.x, self.cursor.y, CURSOR_W as i32, CURSOR_H as i32);
        self.damage(old.union(new));

        if let Some((idx, grab)) = self.dragging {
            if idx < self.windows.len() {
                let frame = self.windows[idx].frame;
                let mut nf = frame.translated(p.x - grab.x - frame.x, p.y - grab.y - frame.y);
                // Keep at least part of the frame on-screen.
                nf.x = nf.x.clamp(-nf.w + 40, self.screen.w - 40);
                nf.y = nf.y.clamp(0, self.screen.h - 24);
                if nf != frame {
                    self.damage(frame);
                    self.damage(nf);
                    self.windows[idx].frame = nf;
                }
            }
        }
    }

    /// Mouse button pressed; returns what was hit.
    pub fn mouse_down(&mut self, p: Point) -> ClickOutcome {
        // Topmost window first.
        for idx in (0..self.windows.len()).rev() {
            match self.windows[idx].hit(p) {
                Some(Hit::Close) => {
                    self.focus(idx);
                    self.close(self.windows.len() - 1);
                    return ClickOutcome::Closed(idx);
                }
                Some(Hit::Title) => {
                    self.focus(idx);
                    let top = self.windows.len() - 1;
                    let f = self.windows[top].frame;
                    self.dragging = Some((top, Point { x: p.x - f.x, y: p.y - f.y }));
                    return ClickOutcome::Focus(idx);
                }
                Some(Hit::Client) => {
                    self.focus(idx);
                    let top = self.windows.len() - 1;
                    let cr = self.windows[top].client_rect();
                    return ClickOutcome::ClientClick(
                        top,
                        Point { x: p.x - cr.x, y: p.y - cr.y },
                    );
                }
                None => continue,
            }
        }
        self.focused = None;
        self.damage(self.taskbar);
        ClickOutcome::Desktop
    }

    pub fn mouse_up(&mut self) {
        self.dragging = None;
    }

    /// Composite all damaged regions into the back buffer and flush to video.
    pub fn composite(&mut self, gop: &mut GraphicsOutput) {
        if self.dirty.is_empty() {
            return;
        }
        let dirty = core::mem::take(&mut self.dirty);
        for r in dirty {
            self.paint_region(r);
            self.flush_rect(gop, r);
        }
    }

    /// Push a back-buffer region to the hardware.
    fn flush_rect(&self, gop: &mut GraphicsOutput, r: Rect) {
        if r.is_empty() {
            return;
        }
        if let Fmt::BltOnly = self.fmt {
            let _ = self.back.to_video(gop, r);
            return;
        }
        let w = self.back.width;
        for y in r.y..r.y + r.h {
            let row = y as usize * self.fb_stride;
            for x in r.x..r.x + r.w {
                let px = self.back.pixels[y as usize * w + x as usize];
                unsafe {
                    core::ptr::write_volatile(
                        self.fb.add(row * 4 + x as usize * 4) as *mut u32,
                        self.fmt.pack(px),
                    );
                }
            }
        }
    }

    fn paint_region(&mut self, r: Rect) {
        // Background.
        self.back.fill_rect(r, DESKTOP);
        // Windows bottom -> top.
        for i in 0..self.windows.len() {
            self.paint_window(i, r);
        }
        // Taskbar on top of windows.
        if !r.intersect(self.taskbar).is_empty() {
            self.paint_taskbar(r);
        }
        // Cursor always last.
        if !r
            .intersect(Rect::new(self.cursor.x, self.cursor.y, CURSOR_W as i32, CURSOR_H as i32))
            .is_empty()
        {
            draw_cursor(&mut self.back, self.cursor.x, self.cursor.y);
        }
    }

    fn paint_window(&mut self, idx: usize, r: Rect) {
        let win = &self.windows[idx];
        let frame_clip = r.intersect(win.frame);
        if frame_clip.is_empty() {
            return;
        }
        let active = self.focused == Some(idx);
        let frame = win.frame;

        // Border: outer ring.
        self.back.fill_rect(Rect::new(frame.x, frame.y, frame.w, BORDER), BORDER_C);
        self.back.fill_rect(
            Rect::new(frame.x, frame.y + frame.h - BORDER, frame.w, BORDER),
            BORDER_C,
        );
        self.back.fill_rect(Rect::new(frame.x, frame.y, BORDER, frame.h), BORDER_C);
        self.back.fill_rect(
            Rect::new(frame.x + frame.w - BORDER, frame.y, BORDER, frame.h),
            BORDER_C,
        );

        // Title bar.
        let tr = win.title_rect().intersect(frame_clip);
        if !tr.is_empty() {
            self.back.fill_rect(
                tr,
                if active { TITLE_ACTIVE } else { TITLE_INACTIVE },
            );
            let title_px = crate::font::text_width(win.title.chars().count(), 2) as i32;
            draw_text(
                &mut self.back,
                (frame.x + BORDER + 6) as usize,
                (frame.y + BORDER + (TITLE_H - CELL_H as i32) / 2) as usize,
                &win.title,
                WHITE,
                2,
            );
            let _ = title_px;
            // Close button.
            if win.closeable {
                let cr = win.close_rect();
                let cr_clip = cr.intersect(r);
                if !cr_clip.is_empty() {
                    self.back.fill_rect(cr, CLOSE_BTN);
                    let cx = cr.x + (CLOSE - 12) / 2;
                    let cy = cr.y + (CLOSE - 12) / 2;
                    for i in 0..12 {
                        self.back.put((cx + i) as usize, (cy + i) as usize, WHITE);
                        self.back.put((cx + 11 - i) as usize, (cy + i) as usize, WHITE);
                    }
                }
            }
        }

        // Content.
        let client = win.client_rect();
        let c_clip = client.intersect(r);
        if !c_clip.is_empty() {
            let src = Rect::new(c_clip.x - client.x, c_clip.y - client.y, c_clip.w, c_clip.h);
            self.back.blit(&win.content, src, (c_clip.x, c_clip.y));
        }
    }

    fn paint_taskbar(&mut self, r: Rect) {
        let bar = self.taskbar.intersect(r);
        self.back.fill_rect(bar, rgb(0x20, 0x20, 0x30));
        self.back.fill_rect(
            Rect::new(bar.x, bar.y, bar.w, 1),
            rgb(0x40, 0x40, 0x60),
        );
        draw_text(&mut self.back, 8, (self.taskbar.y + 8) as usize, "uefios", rgb(0xa0, 0xc0, 0xff), 2);
        if let Some(i) = self.focused {
            let mut label = String::from("  |  ");
            label.push_str(&self.windows[i].title);
            draw_text(&mut self.back, 8 + 8 * 7 * 2, (self.taskbar.y + 8) as usize, &label, TERMINAL_FG, 2);
        }
        if !self.status.is_empty() {
            let w = crate::font::text_width(self.status.chars().count(), 1) as i32;
            draw_text(&mut self.back, (self.screen.w - w - 8) as usize, (self.taskbar.y + 9) as usize, &self.status, TERMINAL_FG, 1);
        }
    }
}
