#![no_std]
#![no_main]

use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use uefi::{boot, println, system};

#[entry]
fn main() -> Status {
    uefi::helpers::init().expect("failed to initialize uefi helpers");

    system::with_stdout(|stdout| {
        let _ = stdout.clear();
    });

    println!("uefios {} -- kernelless OS on UEFI boot services", env!("CARGO_PKG_VERSION"));
    println!();

    // Graphics: locate GOP and report the active mode.
    match boot::get_handle_for_protocol::<GraphicsOutput>()
        .and_then(|handle| boot::open_protocol_exclusive::<GraphicsOutput>(handle))
    {
        Ok(mut gop) => {
            let info = gop.current_mode_info();
            let (w, h) = info.resolution();
            println!("GOP: {}x{} ({} modes available)", w, h, gop.modes().count());
            wm::demo(&mut *gop);
        }
        Err(e) => println!("no GOP found ({:?}) -- text mode only", e.status()),
    }

    // Shell: smoke-test the parser (the interactive REPL comes in phase 1).
    if let Some(parsed) = shell::parse("uname -a") {
        println!("shell parsed command: {:?} -> {:?}", parsed.name(), shell::builtin("uname"));
    }

    // Hypervisor: probe hardware virtualization support.
    println!("VT-x (VMX) supported: {}", hv::vmx_supported());

    println!();
    println!("Boot services retained. uefios stays resident -- this loop becomes the event loop.");
    loop {
        boot::stall(60_000_000);
    }
}
