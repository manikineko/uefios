//! Pointer input.
//!
//! `uefi` 0.35 only wraps the *simple* pointer protocol (relative deltas).
//! QEMU's `usb-tablet` — and most real touchpads/tablets — instead expose the
//! EFI Absolute Pointer protocol, so we bind it ourselves; the GUID is fixed
//! by the UEFI spec.

use crate::geom::Point;
use core::ffi::c_void;
use uefi::proto::console::pointer::Pointer;

/// EFI_STATUS — 0 = success; high bit set = error (e.g. NOT_READY = no new data).
pub type EfiStatus = usize;
pub const EFI_SUCCESS: EfiStatus = 0;

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct AbsolutePointerMode {
    pub absolute_min_x: u64,
    pub absolute_min_y: u64,
    pub absolute_min_z: u64,
    pub absolute_max_x: u64,
    pub absolute_max_y: u64,
    pub absolute_max_z: u64,
    pub attributes: u32,
}

#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct AbsolutePointerState {
    pub current_x: u64,
    pub current_y: u64,
    pub current_z: u64,
    /// bit0: primary (left/touch) pressed, bit1: secondary (right/alt).
    pub active_buttons: u32,
}

type ResetFn = unsafe extern "efiapi" fn(*mut RawAbsolutePointer, bool) -> EfiStatus;
type GetStateFn =
    unsafe extern "efiapi" fn(*mut RawAbsolutePointer, *mut AbsolutePointerState) -> EfiStatus;

#[repr(C)]
pub struct RawAbsolutePointer {
    pub reset: ResetFn,
    pub get_state: GetStateFn,
    pub wait_for_input: *mut c_void,
    pub mode: *mut AbsolutePointerMode,
}

/// EFI_ABSOLUTE_POINTER_PROTOCOL_GUID = 8D59D32B-C655-4AE9-9B15-F25904992A43
#[uefi::proto::unsafe_protocol("8d59d32b-c655-4ae9-9b15-f25904992a43")]
#[repr(transparent)]
pub struct AbsolutePointer(core::cell::UnsafeCell<RawAbsolutePointer>);

impl AbsolutePointer {
    fn raw(&self) -> *mut RawAbsolutePointer {
        self.0.get()
    }

    /// Latest pointer state, if the device reported new data.
    pub fn get_state(&mut self) -> Option<AbsolutePointerState> {
        let mut st = AbsolutePointerState::default();
        let raw = self.raw();
        let status = unsafe { ((*raw).get_state)(raw, &mut st) };
        if status == EFI_SUCCESS {
            Some(st)
        } else {
            None
        }
    }

    pub fn mode(&self) -> AbsolutePointerMode {
        unsafe { (*(*self.raw()).mode).clone() }
    }

    /// Map a raw state to screen coordinates for a `sw`x`sh` display.
    /// Returns (position, primary-button-held).
    pub fn to_screen(&self, st: &AbsolutePointerState, sw: u32, sh: u32) -> (Point, bool) {
        let m = self.mode();
        let map = |v: u64, lo: u64, hi: u64, max: u32| -> i32 {
            let span = hi.saturating_sub(lo).saturating_add(1);
            let rel = v.saturating_sub(lo);
            (rel * max as u64 / span) as i32
        };
        let p = Point {
            x: map(st.current_x, m.absolute_min_x, m.absolute_max_x, sw),
            y: map(st.current_y, m.absolute_min_y, m.absolute_max_y, sh),
        };
        (p, st.active_buttons & 1 != 0)
    }
}

/// Which pointer backend the machine offered.
pub enum PointerDev<'a> {
    /// Absolute-positioned device (usb-tablet, touchscreen): gives screen coords.
    Absolute(&'a mut AbsolutePointer, u32, u32),
    /// Relative device (PS/2 mouse): deltas we integrate ourselves.
    Relative(&'a mut Pointer, Point, u32, u32),
}

impl PointerDev<'_> {
    /// Drain pending samples; returns the latest (position, primary-held).
    pub fn poll(&mut self) -> Option<(Point, bool)> {
        match self {
            PointerDev::Absolute(p, sw, sh) => {
                // Absolute devices may report state on every call — a pure
                // "drain" loop never ends. Bound it; freshest sample wins.
                let mut last = None;
                for _ in 0..8 {
                    match p.get_state() {
                        Some(st) => last = Some(p.to_screen(&st, *sw, *sh)),
                        None => break,
                    }
                }
                last
            }
            PointerDev::Relative(p, pos, sw, sh) => {
                let mut got = None;
                while let Ok(Some(st)) = p.read_state() {
                    pos.x = (pos.x + st.relative_movement[0]).clamp(0, *sw as i32 - 1);
                    pos.y = (pos.y + st.relative_movement[1]).clamp(0, *sh as i32 - 1);
                    got = Some((*pos, st.button[0]));
                }
                got
            }
        }
    }
}
