//! Live framebuffer access for containers: lets a guest's console writes
//! reach the screen while its entry call still blocks our event loop.

use crate::surface::Px;
use core::sync::atomic::{AtomicUsize, Ordering};

static FB: AtomicUsize = AtomicUsize::new(0);
static STRIDE: AtomicUsize = AtomicUsize::new(0);
static FMT: AtomicUsize = AtomicUsize::new(0); // 0=Rgb 1=Bgr

/// Called once by the compositor at init with fb base + pitch + format tag.
pub fn init(fb: *mut u8, stride: usize, fmt: usize) {
    FB.store(fb as usize, Ordering::Relaxed);
    STRIDE.store(stride, Ordering::Relaxed);
    FMT.store(fmt, Ordering::Relaxed);
}

pub fn fb() -> (*mut u8, usize) {
    (FB.load(Ordering::Relaxed) as *mut u8, STRIDE.load(Ordering::Relaxed))
}

fn pack(p: Px) -> u32 {
    if FMT.load(Ordering::Relaxed) == 0 {
        p.red as u32 | (p.green as u32) << 8 | (p.blue as u32) << 16
    } else {
        p.blue as u32 | (p.green as u32) << 8 | (p.red as u32) << 16
    }
}

/// Write one pixel directly to the hardware framebuffer.
pub fn put_px(x: i32, y: i32, c: Px) {
    let (fb, stride) = fb();
    if fb.is_null() || x < 0 || y < 0 {
        return;
    }
    unsafe {
        core::ptr::write_volatile(
            fb.add(y as usize * stride * 4 + x as usize * 4) as *mut u32,
            pack(c),
        );
    }
}
