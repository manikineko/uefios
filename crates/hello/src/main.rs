#![no_std]
#![no_main]

use uefi::prelude::*;
use uefi::println;

/// Guest EFI app: launched by uefios's file manager. Shares ConIn/ConOut
/// and GOP with the host — the OS comes back when we return.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        uefi::boot::stall(60_000_000);
    }
}

#[entry]
fn main() -> Status {
    uefi::helpers::init().ok();
    println!();
    println!("  +-----------------------------------------------+");
    println!("  |  hello.efi — guest app running inside uefios  |");
    println!("  |  loaded via EFI LoadImage / StartImage        |");
    println!("  |  returns to the host desktop on exit          |");
    println!("  +-----------------------------------------------+");
    println!("  (exiting in ~4s)");
    boot::stall(4_000_000);
    Status::SUCCESS
}
