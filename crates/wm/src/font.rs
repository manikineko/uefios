//! Bitmap text rendering (font8x8, integer-scaled).

use crate::surface::{Px, Surface};
use font8x8::UnicodeFonts;

/// Default glyph cell size at 2x scale (8x8 font -> 16x16 cell).
pub const GLYPH_W: usize = 8;
pub const GLYPH_H: usize = 8;
pub const SCALE: usize = 2;
pub const CELL_W: usize = GLYPH_W * SCALE;
pub const CELL_H: usize = GLYPH_H * SCALE;

pub fn glyph(ch: char) -> Option<[u8; 8]> {
    font8x8::BASIC_FONTS.get(ch)
}

/// Draw one glyph at (x, y), `scale`x scaled. LSB = leftmost column.
pub fn draw_glyph(s: &mut Surface, x: usize, y: usize, ch: char, fg: Px, scale: usize) {
    let Some(g) = glyph(ch) else { return };
    for row in 0..GLYPH_H {
        let bits = g[row];
        for col in 0..GLYPH_W {
            if bits & (1 << col) != 0 {
                for dy in 0..scale {
                    for dx in 0..scale {
                        s.put(x + col * scale + dx, y + row * scale + dy, fg);
                    }
                }
            }
        }
    }
}

/// Draw a string; returns the pixel width of a line of `len` chars.
pub fn draw_text(s: &mut Surface, x: usize, y: usize, text: &str, fg: Px, scale: usize) {
    for (i, ch) in text.chars().enumerate() {
        draw_glyph(s, x + i * GLYPH_W * scale, y, ch, fg, scale);
    }
}

pub fn text_width(len: usize, scale: usize) -> usize {
    len * GLYPH_W * scale
}
