#![no_std]

//! Window manager / compositor.
//!
//! The firmware's Graphics Output Protocol (GOP) hands us a linear
//! framebuffer; everything above that — window tree, compositing, damage
//! tracking, input focus — is implemented here in software.
//!
//! All pixel buffers use `BltPixel` (GOP's device-independent format) so the
//! firmware converts to the hardware layout when we flush via
//! `BltOp::BufferToVideo`.

extern crate alloc;

pub mod compositor;
pub mod cursor;
pub mod font;
pub mod geom;
pub mod input;
pub mod surface;
pub mod window;

pub use compositor::{ClickOutcome, Compositor};
pub use geom::{Point, Rect};
pub use input::{AbsolutePointer, PointerDev};
pub use surface::{Px, Surface};
pub use window::{Hit, Window, BORDER, TITLE_H};
