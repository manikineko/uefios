//! Software surface: an owned pixel buffer in GOP `BltPixel` format.
//!
//! Surfaces hold pixels exactly as GOP's blt buffer does (device-independent
//! 8-bit-per-channel), so compositing is a plain memcpy and the firmware
//! handles final pixel-format conversion when we hand the buffer back via
//! `BltOp::BufferToVideo`.

use crate::geom::Rect;
use alloc::vec::Vec;
use uefi::proto::console::gop::BltPixel;

/// Alias for the pixel type everywhere in wm.
pub type Px = BltPixel;

/// Common colors.
pub const fn rgb(r: u8, g: u8, b: u8) -> Px {
    BltPixel::new(r, g, b)
}
pub const BLACK: Px = rgb(0x00, 0x00, 0x00);
pub const WHITE: Px = rgb(0xff, 0xff, 0xff);
pub const DESKTOP: Px = rgb(0x10, 0x10, 0x18);
pub const TITLE_ACTIVE: Px = rgb(0x30, 0x30, 0x48);
pub const TITLE_INACTIVE: Px = rgb(0x24, 0x24, 0x34);
pub const BORDER: Px = rgb(0x0a, 0x0a, 0x10);
pub const TERMINAL_BG: Px = rgb(0x0c, 0x0c, 0x14);
pub const TERMINAL_FG: Px = rgb(0xd0, 0xd8, 0xe0);
pub const CLOSE_BTN: Px = rgb(0xa0, 0x30, 0x30);

#[derive(Debug, Clone)]
pub struct Surface {
    pub width: usize,
    pub height: usize,
    pub pixels: Vec<Px>,
}

impl Surface {
    pub fn new(width: usize, height: usize, fill: Px) -> Self {
        Self {
            width,
            height,
            pixels: alloc::vec![fill; width * height],
        }
    }

    #[inline]
    pub fn put(&mut self, x: usize, y: usize, c: Px) {
        if x < self.width && y < self.height {
            self.pixels[y * self.width + x] = c;
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize) -> Option<Px> {
        if x < self.width && y < self.height {
            Some(self.pixels[y * self.width + x])
        } else {
            None
        }
    }

    pub fn clear(&mut self, c: Px) {
        self.pixels.fill(c);
    }

    /// Fill a rect (surface-local coords), clipped to bounds.
    pub fn fill_rect(&mut self, r: Rect, c: Px) {
        let r = r.intersect(Rect::new(0, 0, self.width as i32, self.height as i32));
        for y in r.y..r.y + r.h {
            for x in r.x..r.x + r.w {
                self.pixels[y as usize * self.width + x as usize] = c;
            }
        }
    }

    /// Copy `src` region `src_rect` into `self` at `dest` (dst-local coords).
    /// Clipped to the destination surface.
    pub fn blit(&mut self, src: &Surface, src_rect: Rect, dest: (i32, i32)) {
        for row in 0..src_rect.h {
            let sy = src_rect.y + row;
            let dy = dest.1 + row;
            if dy < 0 || dy as usize >= self.height || sy < 0 || sy as usize >= src.height {
                continue;
            }
            for col in 0..src_rect.w {
                let sx = src_rect.x + col;
                let dx = dest.0 + col;
                if dx < 0 || dx as usize >= self.width || sx < 0 || sx as usize >= src.width {
                    continue;
                }
                self.pixels[dy as usize * self.width + dx as usize] =
                    src.pixels[sy as usize * src.width + sx as usize];
            }
        }
    }

    /// Copy a rect of this surface to the hardware framebuffer via GOP.
    /// `buf_offset` must be `y * screen_width + x` for the top-left of `r`.
    pub fn to_video(
        &self,
        gop: &mut uefi::proto::console::gop::GraphicsOutput,
        r: Rect,
    ) -> uefi::Result {
        gop.blt(uefi::proto::console::gop::BltOp::BufferToVideo {
            buffer: &self.pixels,
            src: uefi::proto::console::gop::BltRegion::SubRectangle {
                coords: (r.x as usize, r.y as usize),
                px_stride: self.width,
            },
            dest: (r.x as usize, r.y as usize),
            dims: (r.w as usize, r.h as usize),
        })
    }
}
