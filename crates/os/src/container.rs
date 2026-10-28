//! Container: run a guest EFI image with its console redirected into a
//! WM window. We LoadImage the binary, parse its PE entrypoint, then call
//! it directly with a forged EFI_SYSTEM_TABLE whose ConIn/ConOut/StdErr
//! are our proxy protocols â€” the guest's text output lands in a window.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use uefi::proto::loaded_image::LoadedImage;
use uefi::{boot, Handle};

const EFI_SUCCESS: u64 = 0;
const EFI_NOT_READY: u64 = 14;
const EFI_UNSUPPORTED: u64 = 3;

// ---- raw protocol shapes (UEFI spec layouts) ------------------------------

#[repr(C)]
pub struct RawMode {
    max_mode: i32,
    mode: i32,
    attribute: i32,
    cursor_column: i32,
    cursor_row: i32,
    cursor_visible: u8,
}

#[repr(C)]
pub struct RawConOut {
    reset: usize,
    output_string: usize,
    test_string: usize,
    query_mode: usize,
    set_mode: usize,
    set_attribute: usize,
    clear_screen: usize,
    set_cursor_position: usize,
    enable_cursor: usize,
    mode: *mut RawMode,
}

#[repr(C)]
pub struct RawConIn {
    reset: usize,
    read_key_stroke: usize,
    wait_for_key: usize, // EFI_EVENT â€” unsupported (null)
}

#[repr(C)]
struct OutBox {
    proto: RawConOut,
    owner: *mut Container,
}
#[repr(C)]
struct InBox {
    proto: RawConIn,
    owner: *mut Container,
}

// ---- container ------------------------------------------------------------

pub const COLS: usize = 80;
pub const ROWS: usize = 25;

#[repr(C)]
pub struct Container {
    out: OutBox,
    cin: InBox,
    mode: RawMode,
    /// 80x25 UCS-2 cell grid â€” what the window renders.
    pub cells: Vec<u16>,
    col: usize,
    row: usize,
    /// Bumped on output; window re-renders when it changes.
    pub revision: u64,
    /// Where the window's client rect starts on screen (for live blit).
    pub origin: (i32, i32),
    pub done: bool,
    pub status: String,
}

impl Container {
    fn new() -> Self {
        let mut c = Self {
            out: OutBox {
                proto: RawConOut {
                    reset: out_reset as usize,
                    output_string: out_str as usize,
                    test_string: out_test as usize,
                    query_mode: out_query as usize,
                    set_mode: stub_ok_usize as usize,
                    set_attribute: out_set_attr as usize,
                    clear_screen: out_clear as usize,
                    set_cursor_position: out_set_cur as usize,
                    enable_cursor: stub_ok_bool as usize,
                    mode: core::ptr::null_mut(),
                },
                owner: core::ptr::null_mut(),
            },
            cin: InBox {
                proto: RawConIn {
                    reset: stub_ok_bool as usize,
                    read_key_stroke: in_read_key as usize,
                    wait_for_key: 0,
                },
                owner: core::ptr::null_mut(),
            },
            mode: RawMode {
                max_mode: 0,
                mode: 0,
                attribute: 0x07,
                cursor_column: 0,
                cursor_row: 0,
                cursor_visible: 1,
            },
            cells: alloc::vec![0x20u16; COLS * ROWS],
            col: 0,
            row: 0,
            revision: 0,
            origin: (0, 0),
            done: false,
            status: String::new(),
        };
        c.out.proto.mode = &mut c.mode;
        c
    }

    fn scroll(&mut self) {
        self.cells.copy_within(COLS..COLS * ROWS, 0);
        for i in (COLS * (ROWS - 1))..COLS * ROWS {
            self.cells[i] = 0x20;
        }
        self.row = ROWS - 1;
    }

    fn putchar(&mut self, ch: u16) {
        match ch {
            0x0d => self.col = 0,
            0x0a => {
                self.col = 0;
                self.row += 1;
                if self.row >= ROWS {
                    self.scroll();
                }
            }
            0x09 => self.col = (self.col + 8) & !7,
            0x08 => self.col = self.col.saturating_sub(1),
            _ => {
                if self.col >= COLS {
                    self.col = 0;
                    self.row += 1;
                    if self.row >= ROWS {
                        self.scroll();
                    }
                }
                self.cells[self.row * COLS + self.col] = ch;
                self.live_blit_cell(self.col, self.row);
                self.col += 1;
            }
        }
        self.mode.cursor_column = self.col as i32;
        self.mode.cursor_row = self.row as i32;
        self.revision += 1;
    }

    /// Draw one cell's glyph straight to the framebuffer so guest output is
    /// visible while the child still runs (before our next composite pass).
    fn live_blit_cell(&self, cx: usize, cy: usize) {
        let (fb, _stride) = wm::live::fb();
        if fb.is_null() {
            return;
        }
        let ch = self.cells[cy * COLS + cx] as u8;
        let g = wm::font::glyph(ch as char).unwrap_or([0; 8]);
        let ox = self.origin.0 + cx as i32 * 8;
        let oy = self.origin.1 + cy as i32 * 8;
        for gy in 0..8usize {
            for gx in 0..8usize {
                let on = g[gy] & (1 << gx) != 0;
                wm::live::put_px(
                    ox + gx as i32,
                    oy + gy as i32,
                    if on {
                        wm::surface::TERMINAL_FG
                    } else {
                        wm::surface::TERMINAL_BG
                    },
                );
            }
        }
    }
}

// ---- ConOut / ConIn proxy callbacks (extern "efiapi", `this` = box) --------

type OutFn = unsafe extern "efiapi" fn(*mut RawConOut, *const u16) -> u64;

unsafe extern "efiapi" fn out_str(this: *mut RawConOut, s: *const u16) -> u64 {
    if s.is_null() {
        return 3; // INVALID_PARAMETER
    }
    let c = &mut *(*(this as *mut OutBox)).owner;
    let mut p = s;
    while *p != 0 {
        c.putchar(*p);
        p = p.add(1);
    }
    EFI_SUCCESS
}

unsafe extern "efiapi" fn out_reset(_this: *mut RawConOut, _v: u8) -> u64 {
    EFI_SUCCESS
}
unsafe extern "efiapi" fn out_test(_this: *mut RawConOut, _s: *const u16) -> u64 {
    EFI_SUCCESS
}
unsafe extern "efiapi" fn out_query(
    _this: *mut RawConOut,
    _m: usize,
    cols: *mut usize,
    rows: *mut usize,
) -> u64 {
    if !cols.is_null() {
        *cols = COLS;
    }
    if !rows.is_null() {
        *rows = ROWS;
    }
    EFI_SUCCESS
}
unsafe extern "efiapi" fn out_set_attr(this: *mut RawConOut, a: usize) -> u64 {
    (*(*(this as *mut OutBox)).owner).mode.attribute = a as i32;
    EFI_SUCCESS
}
unsafe extern "efiapi" fn out_clear(this: *mut RawConOut) -> u64 {
    let c = &mut *(*(this as *mut OutBox)).owner;
    for cell in c.cells.iter_mut() {
        *cell = 0x20;
    }
    c.col = 0;
    c.row = 0;
    c.revision += 1;
    EFI_SUCCESS
}
unsafe extern "efiapi" fn out_set_cur(this: *mut RawConOut, x: usize, y: usize) -> u64 {
    let c = &mut *(*(this as *mut OutBox)).owner;
    if x >= COLS || y >= ROWS {
        return EFI_UNSUPPORTED;
    }
    c.col = x;
    c.row = y;
    EFI_SUCCESS
}
unsafe extern "efiapi" fn stub_ok_usize(_: *mut RawConOut, _: usize) -> u64 {
    EFI_SUCCESS
}
unsafe extern "efiapi" fn stub_ok_bool(_: *mut RawConOut, _: u8) -> u64 {
    EFI_SUCCESS
}
unsafe extern "efiapi" fn in_read_key(_: *mut RawConIn, _k: *mut u32) -> u64 {
    EFI_NOT_READY
}

// ---- launch ----------------------------------------------------------------

type ImageEntry = unsafe extern "efiapi" fn(Handle, *mut u8) -> u64;

/// PE32+ entry point: image_base + OptionalHeader.AddressOfEntryPoint.
fn pe_entry(image_base: usize) -> Option<usize> {
    unsafe {
        let b = image_base as *const u8;
        let pe_off = u32::from_le(*(b.add(0x3c) as *const u32)) as usize;
        if u32::from_le(*(b.add(pe_off) as *const u32)) != 0x0000_4550 {
            return None; // "PE\0\0"
        }
        let opt = b.add(pe_off + 24);
        let aep = u32::from_le(*(opt.add(0x10) as *const u32)) 
 as usize;
        Some(image_base + aep)
    }
}

/// Forge an EFI_SYSTEM_TABLE: copy the live table, swap console pointers
/// for our proxies. `fake` must outlive the entry call.
fn forge_table(c: *mut Container, fake: *mut u8) {
    let real = uefi::table::system_table_raw().unwrap().as_ptr() as *const u8;
    unsafe {
        core::ptr::copy_nonoverlapping(real, fake, 128);
        let put = |off: usize, v: usize| (fake.add(off) as *mut usize).write(v);
        put(40, 0); // ConsoleInHandle
        put(48, &mut (*c).cin.proto as *mut _ as usize); // ConIn
        put(56, 0); // ConsoleOutHandle
        put(64, &mut (*c).out.proto as *mut _ as usize); // ConOut
        put(72, 0); // StdErrHandle
        put(80, &mut (*c).out.proto as *mut _ as usize); // StdErr
    }
}

/// Run a guest image inside a container. Blocks until the guest exits —
/// its text output streams live into `c.cells` (and the framebuffer).
pub fn run(img: &[u8], c: &mut Container) -> u64 {
    c.out.owner = c;
    c.cin.owner = c;
    let ih = boot::image_handle();
    let child = match boot::load_image(
        ih,
        boot::LoadImageSource::FromBuffer { buffer: img, file_path: None },
    ) {
        Ok(h) => h,
        Err(e) => {
            c.status = alloc::format!("LoadImage failed: {:?}", e.status());
            return 1;
        }
    };
    let base = unsafe {
        boot::open_protocol::<LoadedImage>(
            boot::OpenProtocolParams { handle: child, agent: ih, controller: None },
            boot::OpenProtocolAttributes::GetProtocol,
        )
    }
    .map(|li| li.info().0 as usize)
    .unwrap_or(0);
    let Some(ep) = pe_entry(base) else {
        c.status = String::from("bad PE image");
        return EFI_UNSUPPORTED;
    };
    let mut fake = alloc::vec![0u8; 128];
    forge_table(c, fake.as_mut_ptr());
    let entry: ImageEntry = unsafe { core::mem::transmute(ep) };
    let st = unsafe { entry(child, fake.as_mut_ptr()) };
    c.done = true;
    c.status = alloc::format!("exited: {:#x}", st);
    c.revision += 1;
    st
}

/// Heap-allocate a fresh container (leaked — EFI-era lifetime).
pub fn spawn() -> &'static mut Container {
    Box::leak(Box::new(Container::new()))
}
