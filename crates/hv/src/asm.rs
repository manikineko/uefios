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

/// vmxoff — leave VMX root operation entirely.
#[inline]
pub unsafe fn vmxoff() {
    core::arch::asm!("vmxoff", options(nostack));
}

/// INVEPT. `ty` = 1 (single-context) or 2 (all-context); `eptp` is the
/// 128-bit invalidation descriptor (EPTP + reserved).
#[inline]
pub unsafe fn invept(ty: u64, eptp: u64) -> Result<(), ()> {
    let desc: [u64; 2] = [eptp, 0];
    let flags: u64;
    core::arch::asm!(
        "invept {t}, [{p}]",
        "pushfq",
        "pop {fl:r}",
        t = in(reg) ty,
        p = in(reg) &desc,
        fl = out(reg) flags,
        options(nostack),
    );
    if flags & 0x41 == 0 { Ok(()) } else { Err(()) }
}

/// INVVPID. `ty` = 0..4, `vpid`/`gva` form the 128-bit descriptor.
#[inline]
pub unsafe fn invvpid(ty: u64, vpid: u64, gva: u64) -> Result<(), ()> {
    let desc: [u64; 2] = [vpid, gva];
    let flags: u64;
    core::arch::asm!(
        "invvpid {t}, [{p}]",
        "pushfq",
        "pop {fl:r}",
        t = in(reg) ty,
        p = in(reg) &desc,
        fl = out(reg) flags,
        options(nostack),
    );
    if flags & 0x41 == 0 { Ok(()) } else { Err(()) }
}

// ---------------- ordinary CPU plumbing ----------------

#[inline]
pub fn rdmsr(msr: u32) -> u64 {
    let lo: u32;
    let hi: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") msr, out("eax") lo, out("edx") hi)
    };
    ((hi as u64) << 32) | lo as u64
}

#[inline]
pub fn wrmsr(msr: u32, v: u64) {
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") msr,
            in("eax") v as u32,
            in("edx") (v >> 32) as u32,
            options(nostack),
        )
    }
}

/// Read a control register (CR0/CR3/CR4) via mov.
#[inline]
pub unsafe fn read_cr(n: u8) -> u64 {
    let v: u64;
    match n {
        0 => core::arch::asm!("mov {}, cr0", out(reg) v),
        3 => core::arch::asm!("mov {}, cr3", out(reg) v),
        4 => core::arch::asm!("mov {}, cr4", out(reg) v),
        _ => unreachable!(),
    }
    v
}

#[inline]
pub unsafe fn write_cr4(v: u64) {
    core::arch::asm!("mov cr4, {}", in(reg) v, options(nostack));
}

#[inline]
pub fn read_eflags() -> u64 {
    let v: u64;
    unsafe { core::arch::asm!("pushfq", "pop {}", out(reg) v, options(nostack)) };
    v
}

/// Read a segment selector register.
macro_rules! read_seg {
    ($name:ident, $reg:literal) => {
        #[inline]
        pub fn $name() -> u16 {
            let v: u16;
            unsafe { core::arch::asm!(concat!("mov {:x}, ", $reg), out(reg) v, options(nostack)) };
            v
        }
    };
}
read_seg!(read_es, "es");
read_seg!(read_cs, "cs");
read_seg!(read_ss, "ss");
read_seg!(read_ds, "ds");
read_seg!(read_fs, "fs");
read_seg!(read_gs, "gs");

/// str — read the task register selector.
#[inline]
pub fn read_tr() -> u16 {
    let v: u16;
    unsafe { core::arch::asm!("str {:x}", out(reg) v, options(nostack)) };
    v
}

/// sldt — read the LDTR selector.
#[inline]
pub fn read_ldtr() -> u16 {
    let v: u16;
    unsafe { core::arch::asm!("sldt {:x}", out(reg) v, options(nostack)) };
    v
}

/// ltr — load the task register from a GDT selector.
#[inline]
pub fn write_tr(sel: u16) {
    unsafe { core::arch::asm!("ltr {:x}", in(reg) sel, options(nostack)) };
}

/// lar — load access rights for `sel`. Returns the shifted-right access
/// byte (descriptor bits 8..23) or `None` if the load failed (bad sel).
#[inline]
pub fn lar(sel: u16) -> Option<u64> {
    let v: u64;
    let flags: u64;
    unsafe {
        core::arch::asm!(
            "lar {v}, {s:x}",
            "pushfq",
            "pop {fl:r}",
            v = out(reg) v,
            s = in(reg) sel as u64,
            fl = out(reg) flags,
            options(nostack),
        )
    };
    if flags & (1 << 6) != 0 { Some(v) } else { None }
}

/// lsl — load segment limit for `sel`, or `None` on failure.
#[inline]
pub fn lsl(sel: u16) -> Option<u64> {
    let v: u64;
    let flags: u64;
    unsafe {
        core::arch::asm!(
            "lsl {v}, {s:x}",
            "pushfq",
            "pop {fl:r}",
            v = out(reg) v,
            s = in(reg) sel as u64,
            fl = out(reg) flags,
            options(nostack),
        )
    };
    if flags & (1 << 6) != 0 { Some(v) } else { None }
}

#[repr(C, packed)]
pub struct TableReg {
    pub limit: u16,
    pub base: u64,
}

/// sgdt — current GDTR contents.
#[inline]
pub fn sgdt() -> TableReg {
    let mut t = TableReg { limit: 0, base: 0 };
    unsafe { core::arch::asm!("sgdt [{}]", in(reg) &mut t, options(nostack)) };
    t
}

/// sidt — current IDTR contents.
#[inline]
pub fn sidt() -> TableReg {
    let mut t = TableReg { limit: 0, base: 0 };
    unsafe { core::arch::asm!("sidt [{}]", in(reg) &mut t, options(nostack)) };
    t
}

/// lgdt with explicit base/limit (6-byte pseudo-descriptor).
#[inline]
pub fn lgdt(base: u64, limit: u16) {
    let t = TableReg { limit, base };
    unsafe { core::arch::asm!("lgdt [{}]", in(reg) &t, options(nostack)) };
}

/// lidt with explicit base/limit.
#[inline]
pub fn lidt(base: u64, limit: u16) {
    let t = TableReg { limit, base };
    unsafe { core::arch::asm!("lidt [{}]", in(reg) &t, options(nostack)) };
}

/// cpuid leaf/subleaf — returns (eax, ebx, ecx, edx).
#[inline]
pub fn cpuid(leaf: u32, sub: u32) -> (u32, u32, u32, u32) {
    let (a, b, c, d): (u32, u32, u32, u32);
    unsafe {
        core::arch::asm!(
            "cpuid",
            "mov {b_tmp:e}, ebx",
            inout("eax") leaf => a,
            b_tmp = out(reg) b,
            inout("ecx") sub => c,
            out("edx") d,
            options(nostack),
        )
    };
    (a, b, c, d)
}

/// xsetbv — write XCR0 (or other xcr) — host-side helper for XSETBV exits.
#[inline]
pub unsafe fn xsetbv(xcr: u32, v: u64) {
    core::arch::asm!(
        "xsetbv",
        in("ecx") xcr,
        in("eax") v as u32,
        in("edx") (v >> 32) as u32,
        options(nostack),
    );
}
