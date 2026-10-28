//! `hvtest` — VT-x smoke test: bring the CPU into VMX root mode and run
//! the tiny asm guest (CPUID -> PING hypercall -> STOP hypercall).
//!
//! All HV pages are allocated below 4 GiB: the identity-map EPT covers
//! only low memory and UEFI keeps VA == PA. Pages are leaked on purpose
//! — the private GDT/TSS stays installed after we return.

use alloc::format;
use alloc::string::String;
use core::ptr::NonNull;
use uefi::boot::{self, AllocateType, MemoryType};

const HV_STACK_PAGES: usize = 8; // 32 KiB VM-exit stack
const GUEST_STACK_PAGES: usize = 4; // 16 KiB guest stack

struct Pages {
    cpu: Option<NonNull<u8>>,
    ept: Option<NonNull<u8>>,
    bitmap: Option<NonNull<u8>>,
    tables: Option<NonNull<u8>>,
    hv_stack: Option<NonNull<u8>>,
    guest_stack: Option<NonNull<u8>>,
}

static mut PAGES: Option<Pages> = None;

fn pages(n: usize) -> Option<NonNull<u8>> {
    boot::allocate_pages(
        AllocateType::MaxAddress(0xffff_ffff),
        MemoryType::LOADER_DATA,
        n,
    )
    .ok()
}

fn get_pages() -> Result<&'static Pages, String> {
    unsafe {
        let slot = &mut *core::ptr::addr_of_mut!(PAGES);
        if slot.is_none() {
            let p = Pages {
                cpu: pages(hv::CPU_PAGES),
                ept: pages(hv::ept::EptTables::PAGES),
                bitmap: pages(1),
                tables: pages(2), // GDT+TSS page + cloned-IDT page
                hv_stack: pages(HV_STACK_PAGES),
                guest_stack: pages(GUEST_STACK_PAGES),
            };
            if p.cpu.is_none()
                || p.ept.is_none()
                || p.bitmap.is_none()
                || p.tables.is_none()
                || p.hv_stack.is_none()
                || p.guest_stack.is_none()
            {
                return Err(String::from("hvtest: AllocatePages <4G failed"));
            }
            *slot = Some(p);
        }
        Ok(slot.as_ref().unwrap())
    }
}

/// Run the smoke guest. Returns human-readable output for the shell.
pub fn run() -> String {
    if !hv::probe().vmx {
        return String::from(
            "hvtest: CPU has no VT-x (QEMU/TCG can't emulate it; need -cpu host or bare metal)",
        );
    }
    let p = match get_pages() {
        Ok(p) => p,
        Err(e) => return e,
    };
    let mm = hv::Mmap {
        cpu: p.cpu.unwrap().as_ptr() as *mut hv::host::VmxCpu,
        ept: p.ept.unwrap().as_ptr() as *mut hv::ept::EptTables,
        msr_bitmap: p.bitmap.unwrap().as_ptr(),
        tables: p.tables.unwrap().as_ptr(),
        hv_stack_top: p.hv_stack.unwrap().as_ptr() as u64 + (HV_STACK_PAGES * 4096) as u64,
        guest_stack_top: p.guest_stack.unwrap().as_ptr() as u64
            + (GUEST_STACK_PAGES * 4096) as u64,
    };
    let entry = hv::guest::uefios_guest_entry as *const () as usize as u64;
    match unsafe { hv::launch(entry, &mm) } {
        Ok(r) => format!(
            "hvtest: guest returned after {} exit(s) \
             (last: reason={} rip={:#x} gpa={:#x})",
            r.exits, r.reason, r.rip, r.gpa
        ),
        Err(e) => {
            let r = hv::exit::report();
            format!(
                "hvtest: {:?} (vmfail_flags={:#x} instr_err={:#x})",
                e,
                hv::exit::vmfail_flags(),
                r.vm_instr_err
            )
        }
    }
}
