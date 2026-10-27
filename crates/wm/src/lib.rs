#![no_std]

//! Window manager / compositor.
//!
//! The firmware's Graphics Output Protocol (GOP) hands us a linear
//! framebuffer; everything above that — window tree, compositing, damage
//! tracking, input focus — is implemented here in software.
//!
//! Roadmap for this crate:
//! - `Surface`: owned back-buffer per window (double-buffered blits)
//! - `WindowTree`: z-ordered windows, dirty-rect tracking
//! - `Compositor`: merge surfaces into the GOP framebuffer
//! - Input routing via the Simple Pointer / Absolute Pointer protocols

use uefi::proto::console::gop::{BltOp, BltPixel, GraphicsOutput};

/// Fill a rectangle on the framebuffer via the GOP blt service.
pub fn fill_rect(
    gop: &mut GraphicsOutput,
    dest: (usize, usize),
    dims: (usize, usize),
    color: BltPixel,
) -> uefi::Result {
    gop.blt(BltOp::VideoFill { color, dest, dims })
}

/// Fill the entire framebuffer with a single color.
pub fn clear(gop: &mut GraphicsOutput, color: BltPixel) -> uefi::Result {
    let (w, h) = gop.current_mode_info().resolution();
    fill_rect(gop, (0, 0), (w, h), color)
}

/// Proof-of-life desktop paint: background, a "window", and a taskbar band.
/// Replaced by the real compositor once `Surface` exists.
pub fn demo(gop: &mut GraphicsOutput) {
    let (w, h) = gop.current_mode_info().resolution();

    let _ = clear(gop, BltPixel::new(0x10, 0x10, 0x18));

    let _ = fill_rect(
        gop,
        (w / 4, h / 4),
        (w / 2, h / 2),
        BltPixel::new(0xe0, 0xe0, 0xe0),
    );

    let bar = 48usize.min(h);
    let _ = fill_rect(
        gop,
        (0, h - bar),
        (w, bar),
        BltPixel::new(0x30, 0x30, 0x48),
    );
}
