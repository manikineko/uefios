#![no_std]

#![no_main]



extern crate alloc;



use alloc::string::String;

use alloc::vec::Vec;

use shell::{Shell, ShellAction, ShellKey};

use uefi::prelude::*;

use uefi::proto::console::gop::GraphicsOutput;

use uefi::proto::console::pointer::Pointer;

use uefi::proto::console::text::{Key, ScanCode};

use uefi::{boot, println, system};

use uefi::proto::media::fs::SimpleFileSystem;

use wm::input::PointerDev;

use wm::{AbsolutePointer, Compositor, Point, Rect, Window};



mod container;
mod fileman;

use container::Container;
use fileman::FileManager;



const TERM_ID: usize = 0;

const FM_ID: usize = 1;
const CONT_ID_BASE: usize = 100;



fn term_idx(comp: &Compositor) -> Option<usize> {

    comp.windows.iter().position(|w| w.id == TERM_ID)

}

fn fm_idx(comp: &Compositor) -> Option<usize> {

    comp.windows.iter().position(|w| w.id == FM_ID)

}

const PAD: usize = 6;



/// Render shell scrollback + prompt into a window's client surface at 2x.

fn render_term(win: &mut Window, sh: &Shell, rows: usize) {

    let surf = &mut win.content;

    surf.clear(wm::surface::TERMINAL_BG);

    let mut lines = Vec::new();

    sh.render(rows, &mut lines);

    for (i, l) in lines.iter().enumerate() {

        wm::font::draw_text(surf, PAD, PAD + i * (wm::font::CELL_H + 2), l, wm::surface::TERMINAL_FG, 2);

    }

    win.content_revision += 1;

}



/// Paint the "about" demo window: gradient + labels.

fn paint_about(win: &mut Window) {

    let s = &mut win.content;

    for y in 0..s.height {

        for x in 0..s.width {

            let c = wm::surface::rgb(

                (x * 255 / s.width.max(1)) as u8,

                0x30,

                (y * 255 / s.height.max(1)) as u8,

            );

            s.put(x, y, c);

        }

    }

    wm::font::draw_text(s, 16, 16, "uefios", wm::surface::WHITE, 3);

    wm::font::draw_text(s, 16, 16 + 8 * 3 + 8, "kernelless OS on UEFI", wm::surface::WHITE, 1);

    win.content_revision += 1;

}



/// Paint a guest container's 80x25 console buffer + exit status.
fn render_container(win: &mut Window, c: &Container) {
    let s = &mut win.content;
    s.clear(wm::surface::TERMINAL_BG);
    for row in 0..container::ROWS {
        for col in 0..container::COLS {
            let ch = c.cells[row * container::COLS + col] as u8;
            if ch != 0x20 && ch != 0 {
                wm::font::draw_glyph(
                    s,
                    col * 8,
                    row * 8,
                    ch as char,
                    wm::surface::TERMINAL_FG,
                    1,
                );
            }
        }
    }
    if c.done {
        wm::font::draw_text(
            s,
            4,
            s.height - 14,
            &c.status,
            wm::surface::rgb(0x88, 0x88, 0x99),
            1,
        );
    }
    win.content_revision += 1;
}

/// Paint the file-manager window: path bar, entry list, status line.

fn render_fm(win: &mut Window, fm: &FileManager) {

    let s = &mut win.content;

    s.clear(wm::surface::TERMINAL_BG);

    wm::font::draw_text(s, PAD, PAD, &fm.path_str(), wm::surface::rgb(0x9a, 0xa0, 0xff), 1);

    let rowh = wm::font::CELL_H + 4;

    let vis = (s.height.saturating_sub(PAD * 2 + 26)) / rowh;

    for (i, e) in fm.entries.iter().enumerate().take(vis) {

        let y = PAD + 14 + i * rowh;

        let label = if e.is_dir {

            alloc::format!("[D] {}", e.name)

        } else {

            alloc::format!("    {}", e.name)

        };

        if i == fm.sel {

            s.fill_rect(Rect::new(4, y as i32 - 2, s.width as i32 - 8, rowh as i32),

                        wm::surface::rgb(0x40, 0x44, 0x7c));

            wm::font::draw_text(s, PAD, y, &label, wm::surface::WHITE, 1);

        } else {

            wm::font::draw_text(s, PAD, y, &label, wm::surface::TERMINAL_FG, 1);

        }

    }

    wm::font::draw_text(s, PAD, s.height - 16, &fm.msg, wm::surface::rgb(0x88, 0x88, 0x99), 1);

    win.content_revision += 1;

}



#[panic_handler]

fn panic(info: &core::panic::PanicInfo) -> ! {

    // Keep the OS alive and the message on the console — a panic is not a

    // shutdown request. (OVMF routes stderr to serial, so never print there.)

    println!("PANIC: {}", info);

    loop {

        boot::stall(60_000_000);

    }

}



#[entry]

fn main() -> Status {

    uefi::helpers::init().expect("uefi helpers init");



    system::with_stdout(|o| {

        let _ = o.clear();

    });

    println!("uefios {} -- starting GUI", env!("CARGO_PKG_VERSION"));



    // --- Graphics ------------------------------------------------------------

    let ih = boot::image_handle();

    let h = boot::get_handle_for_protocol::<GraphicsOutput>();

    let mut gop = match h.and_then(|h| unsafe {

        boot::open_protocol::<GraphicsOutput>(

            boot::OpenProtocolParams {

                handle: h,

                agent: ih,

                controller: None,

            },

            boot::OpenProtocolAttributes::GetProtocol,

        )

    }) {

        Ok(g) => g,

        Err(e) => {

            println!("no GOP ({:?}); staying on text console", e.status());

            loop {

                boot::stall(60_000_000);

            }

        }

    };

    let (sw, sh) = gop.current_mode_info().resolution();

    println!("GOP {}x{}", sw, sh);



    // --- Pointer: absolute (usb-tablet) preferred, relative (PS/2) fallback --

    let open = |h: Handle| boot::OpenProtocolParams {

        handle: h,

        agent: ih,

        controller: None,

    };

    let mut abs = boot::get_handle_for_protocol::<AbsolutePointer>()

        .and_then(|h| unsafe {

            boot::open_protocol::<AbsolutePointer>(open(h), boot::OpenProtocolAttributes::GetProtocol)

        })

        .ok();

    let mut rel = if abs.is_none() {

        boot::get_handle_for_protocol::<Pointer>()

            .and_then(|h| unsafe {

                boot::open_protocol::<Pointer>(open(h), boot::OpenProtocolAttributes::GetProtocol)

            })

            .ok()

    } else {

        None

    };

    let mut pdev = match (&mut abs, &mut rel) {

        (Some(p), _) => Some(PointerDev::Absolute(&mut **p, sw as u32, sh as u32)),

        (None, Some(p)) => Some(PointerDev::Relative(

            &mut **p,

            Point { x: sw as i32 / 2, y: sh as i32 / 2 },

            sw as u32,

            sh as u32,

        )),

        _ => None,

    };



    // --- Windows -------------------------------------------------------------

    let mut comp = Compositor::new(&mut gop);



    let mut shell = Shell::new(String::from(

        "OVMF edk2 (QEMU); services: GOP blk-net; VT-x: probing at hv bring-up",

    ));

    let cols = 64usize;

    let rows = 20usize;

    let mut term = Window::new(

        "shell",

        40,

        40,

        (PAD * 2 + cols * wm::font::CELL_W) as i32,

        (PAD * 2 + rows * (wm::font::CELL_H + 2)) as i32,

    );

    render_term(&mut term, &shell, rows);

    term.id = TERM_ID;

    comp.add_window(term);



    let mut about = Window::new("about uefios", sw as i32 - 420, 120, 360, 160);

    paint_about(&mut about);

    comp.add_window(about);



    // File manager: browse the ESP, launch .efi images (app loader v0).

    let mut fm = boot::get_handle_for_protocol::<SimpleFileSystem>()

        .and_then(|h| unsafe {

            boot::open_protocol::<SimpleFileSystem>(

                boot::OpenProtocolParams { handle: h, agent: ih, controller: None },

                boot::OpenProtocolAttributes::GetProtocol,

            )

        })

        .ok()

        .map(uefi::fs::FileSystem::new)

        .map(FileManager::new);

    if let Some(f) = fm.as_mut() {

        let mut w = Window::new("file manager", sw as i32 - 640, 480, 560, 280);

        w.id = FM_ID;

        render_fm(&mut w, f);

        comp.add_window(w);

    }



    comp.focus(term_idx(&comp).unwrap());



    // Smoke-test: run `uname` through the real feed->render pipeline so the

    // terminal proves itself even without hardware input.

    for c in "uname".chars() {

        shell.feed(ShellKey::Char(c));

    }

    shell.feed(ShellKey::Enter);



    let mut last_term_rev = 0u64;

    let mut last_fm_rev = 0u64;

    let mut containers: Vec<(&'static mut Container, usize, u64)> = Vec::new();

    let mut prev_held = false;

    let mut mdowns = 0u32;

    let mut nkeys = 0u32;

    let mut last_pos = Point { x: 0, y: 0 };



    // --- Main loop ------------------------------------------------------------

    loop {

        // Keyboard -> focused window.

        let keys: Vec<ShellKey> = system::with_stdin(|stdin| {

            let mut v = Vec::new();

            while let Ok(Some(k)) = stdin.read_key() {

                let sk = match k {

                    Key::Printable(c) => match u16::from(c) {

                        0x0d => Some(ShellKey::Enter),

                        0x08 => Some(ShellKey::Backspace),

                        u => char::from_u32(u as u32).map(ShellKey::Char),

                    },

                    Key::Special(ScanCode::UP) => Some(ShellKey::HistoryUp),

                    Key::Special(ScanCode::DOWN) => Some(ShellKey::HistoryDown),

                    _ => None,

                };

                if let Some(sk) = sk {

                    v.push(sk);

                }

            }

            v

        });

        nkeys += keys.len() as u32;

        let fid = comp.focused().map(|i| comp.windows[i].id);

        for sk in keys {

            match fid {

                Some(TERM_ID) => match shell.feed(sk) {

                    ShellAction::Clear => shell.out.clear(),

                    ShellAction::Reboot => {

                        uefi::runtime::reset(uefi::runtime::ResetType::COLD, Status::SUCCESS, None)

                    }

                    _ => {}

                },

                Some(FM_ID) => {

                    if let Some(f) = fm.as_mut() {

                        f.feed(sk);

                    }

                }

                _ => {}

            }

        }

        if let Some(f) = fm.as_mut() {

            if let Some(path) = f.take_launch() {

                if let Some(bytes) = f.read_file(&path) {

                    let c = container::spawn();

                    let n = containers.len();

                    let mut w = Window::new(

                        "guest",

                        120 + (n as i32 % 4) * 30,

                        60 + (n as i32 % 4) * 30,

                        (container::COLS * 8 + 12) as i32,

                        (container::ROWS * 8 + 24) as i32,

                    );

                    let wid = CONT_ID_BASE + n;
                    w.id = wid;

                    c.origin = (w.client_rect().x, w.client_rect().y);

                    comp.add_window(w);

                    comp.focus(comp.windows.len() - 1);

                    container::run(&bytes, c);

                    containers.push((c, wid, 0u64));

                }

            }

        }



        // Pointer.

        if let Some(pd) = pdev.as_mut() {

            if let Some((pos, held)) = pd.poll() {

                last_pos = pos;

                comp.mouse_move(pos);

                if held && !prev_held {

                    mdowns += 1;

                    let _ = comp.mouse_down(pos);

                } else if !held && prev_held {

                    comp.mouse_up();

                }

                prev_held = held;

            }

            comp.set_status(alloc::format!("k={} p={},{} h={} md={}", nkeys, last_pos.x, last_pos.y, prev_held as u8, mdowns));

        }



        if shell.revision != last_term_rev {

            if let Some(ti) = term_idx(&comp) {

                render_term(&mut comp.windows[ti], &shell, rows);

                last_term_rev = shell.revision;

                let cr = comp.windows[ti].client_rect();

                comp.damage(cr);

            }

        }

        if let Some(f) = fm.as_mut() {

            if f.revision != last_fm_rev {

                if let Some(i) = fm_idx(&comp) {

                    render_fm(&mut comp.windows[i], f);

                    last_fm_rev = f.revision;

                    let cr = comp.windows[i].client_rect();

                    comp.damage(cr);

                }

            }

        }



        for (c, wid, lr) in containers.iter_mut() {
            if c.revision != *lr {
                if let Some(i) = comp.windows.iter().position(|w| w.id == *wid) {
                    render_container(&mut comp.windows[i], c);
                    *lr = c.revision;
                    let cr = comp.windows[i].client_rect();
                    comp.damage(cr);
                }
            }
        }

        comp.composite(&mut gop);



        // Frame pacing without firmware: OVMF's Stall() spins on the ACPI

        // PM timer which does not tick reliably under QEMU/TCG, freezing

        // the loop -- a spin loop is enough (we redraw on damage anyway).

        for _ in 0..400_000 {

            core::hint::spin_loop();

        }

    }

}

