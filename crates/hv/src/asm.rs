//! VT-x instruction wrappers — Rust `asm!` ports of MiniVisor's Asm.asm.
//! VMXON/VMCLEAR/VMPTRLD/VMPTRST take an *m64 memory operand* holding the
//! 64-bit physical address. CF => VMfailInvalid, ZF => VMfailValid.

#![allow(dead_code)]

macro_rules! vmx_m64 {
    ($name:ident, $mn:literal, $arg:ident) => {
        /// `$mn [pa_slot]` — executes on the physical address stored at `pa`.
        #[inline]
        pub unsafe fn $name(pa: u64) -> Result<(), (bool, bool)> {
            let pav = pa;
            let flags: u64;
            core::arch::asm!(
                concat!($mn, " qword ptr [{p}]"),
                "pushfq",
                "pop {fl:r}",
                p = in(reg) &pav,
                fl = out(reg) flags,
                options(nostack),
            );
            let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
            if !cf && !zf { Ok(()) } else { Err((cf, zf)) }
        }
    };
}

vmx_m64!(vmxon, "vmxon", pa);
vmx_m64!(vmclear, "vmclear", pa);
vmx_m64!(vmptrld, "vmptrld", pa);

/// vmptrst [slot] — stores the *current* VMCS's physical address.
#[inline]
pub unsafe fn vmptrst() -> Result<u64, (bool, bool)> {
    let mut slot: u64 = 0;
    let flags: u64;
    core::arch::asm!(
        "vmptrst qword ptr [{p}]",
        "pushfq",
        "pop {fl:r}",
        p = in(reg) &mut slot,
        fl = out(reg) flags,
        options(nostack),
    );
    let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
    if !cf && !zf { Ok(slot) } else { Err((cf, zf)) }
}

/// vmread: dest=value, src=field encoding.
#[inline]
pub unsafe fn vmread(field: u16) -> Result<u64, (bool, bool)> {
    let val: u64;
    let flags: u64;
    core::arch::asm!(
        "vmread {v}, {f}",
        "pushfq",
        "pop {fl:r}",
        v = out(reg) val,
        f = in(reg) field as u64,
        fl = out(reg) flags,
        options(nostack),
    );
    let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
    if !cf && !zf { Ok(val) } else { Err((cf, zf)) }
}

/// vmwrite: dest=value, src=field encoding.
#[inline]
pub unsafe fn vmwrite(field: u16, val: u64) -> Result<(), (bool, bool)> {
    let flags: u64;
    core::arch::asm!(
        "vmwrite {v}, {f}",
        "pushfq",
        "pop {fl:r}",
        v = in(reg) val,
        f = in(reg) field as u64,
        fl = out(reg) flags,
        options(nostack),
    );
    let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
    if !cf && !zf { Ok(()) } else { Err((cf, zf)) }
}

/// vmlaunch — returns Err(flags) only on VMfail; success means the CPU is
/// now executing guest code (we come back via a VMExit elsewhere).
#[inline]
pub unsafe fn vmlaunch() -> Result<(), (bool, bool)> {
    let flags: u64;
    core::arch::asm!(
        "vmlaunch",
        "pushfq",
        "pop {fl:r}",
        fl = out(reg) flags,
        options(nostack),
    );
    let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
    if !cf && !zf { Ok(()) } else { Err((cf, zf)) }
}

/// vmresume — same contract as vmlaunch for re-entry.
#[inline]
pub unsafe fn vmresume() -> Result<(), (bool, bool)> {
    let flags: u64;
    core::arch::asm!(
        "vmresume",
        "pushfq",
        "pop {fl:r}",
        fl = out(reg) flags,
        options(nostack),
    );
    let (cf, zf) = (flags & 1 != 0, flags & (1 << 6) != 0);
    if !cf && !zf { Ok(()) } else { Err((cf, zf)) }
}

/// vmcall — hypercall from non-root guest context.
#[inline]
pub unsafe fn vmcall(nr: u64, a: u64, b: u64, c: u64) -> u64 {
    let ret: u64;
    core::arch::asm!(
        "vmcall",
        inout("rax") nr => ret,
        in("rcx") a,
        in("rdx") b,
        in("rsi") c,
        options(nostack),
    );
    ret
}
