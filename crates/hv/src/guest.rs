//! Guest-state VMCS fields + a minimal first-launch context.
//!
//! Model: a 64-bit guest that is a *clone* of the host CPU state (the
//! MiniVisor approach), except RIP/RSP point at our own entry + stack.
//! UEFI is identity-mapped so guest code can live in the image.

#![allow(dead_code)]

use crate::asm;
use crate::host;
use crate::vmcs;

/// Everything [`write_guest_fields`] needs. `rip`/`rsp` are the guest
/// entry point and stack top (both must be < [`crate::ept::COVERED_GPA`]
/// while our EPT only identity-maps low memory).
pub struct GuestInit {
    pub rip: u64,
    pub rsp: u64,
    /// TR selector allocated by [`crate::host::install_tables`].
    pub tr_sel: u16,
    /// Base of the private GDT (also used for descriptor-base reads).
    /// The cloned GDT+TSS occupies page 0; the cloned IDT is at
    /// `gdt_base + 0x1000` (see [`crate::host::install_tables`]).
    pub gdt_base: u64,
}

/// Write every guest-state field a 64-bit guest needs (SDM 26.3 checks).
/// Segment registers clone the current CPU — which is exactly what the
/// guest should start with — except TR, which must point at a real TSS
/// descriptor (UEFI typically has none loaded).
pub unsafe fn write_guest_fields(g: &GuestInit) -> bool {
    let (es, cs, ss, ds, fs, gs) = (
        asm::read_es(),
        asm::read_cs(),
        asm::read_ss(),
        asm::read_ds(),
        asm::read_fs(),
        asm::read_gs(),
    );
    let ldtr = asm::read_ldtr();
    // install_tables() already lgdt'd our private GDT — sgdt() returns
    // it (base = gdt_base, limit = old + one descriptor).
    let gdtr = asm::sgdt();
    let idtr = asm::sidt();

    let writes: [(u16, u64); 61] = [
        // 16-bit: selectors
        (vmcs::GUEST_ES_SELECTOR, es as u64),
        (vmcs::GUEST_CS_SELECTOR, cs as u64),
        (vmcs::GUEST_SS_SELECTOR, ss as u64),
        (vmcs::GUEST_DS_SELECTOR, ds as u64),
        (vmcs::GUEST_FS_SELECTOR, fs as u64),
        (vmcs::GUEST_GS_SELECTOR, gs as u64),
        (vmcs::GUEST_LDTR_SELECTOR, ldtr as u64),
        (vmcs::GUEST_TR_SELECTOR, g.tr_sel as u64),
        // 64-bit: link pointer + MSRs
        (vmcs::VMCS_LINK_POINTER, u64::MAX),
        (vmcs::GUEST_EFER, asm::rdmsr(vmcs::IA32_EFER)),
        (vmcs::GUEST_PAT, asm::rdmsr(vmcs::IA32_PAT)),
        (vmcs::GUEST_DEBUGCTL, asm::rdmsr(0x1d9)),
        (vmcs::GUEST_PERF_GLOBAL_CTRL, asm::rdmsr(vmcs::IA32_PERF_GLOBAL_CTRL)),
        (vmcs::GUEST_PDPTE0, 0),
        (vmcs::GUEST_PDPTE0 + 2, 0),
        (vmcs::GUEST_PDPTE0 + 4, 0),
        (vmcs::GUEST_PDPTE0 + 6, 0),
        (vmcs::GUEST_BNDCFGS, 0),
        // 32-bit: limits
        (vmcs::GUEST_ES_LIMIT, host::seg_limit(es)),
        (vmcs::GUEST_CS_LIMIT, host::seg_limit(cs)),
        (vmcs::GUEST_SS_LIMIT, host::seg_limit(ss)),
        (vmcs::GUEST_DS_LIMIT, host::seg_limit(ds)),
        (vmcs::GUEST_FS_LIMIT, host::seg_limit(fs)),
        (vmcs::GUEST_GS_LIMIT, host::seg_limit(gs)),
        (vmcs::GUEST_LDTR_LIMIT, host::seg_limit(ldtr)),
        (vmcs::GUEST_TR_LIMIT, host::seg_limit(g.tr_sel)),
        (vmcs::GUEST_GDTR_LIMIT, gdtr.limit as u64),
        (vmcs::GUEST_IDTR_LIMIT, idtr.limit as u64),
        // 32-bit: access rights
        (vmcs::GUEST_ES_ACCESS_RIGHTS, host::seg_access(es) as u64),
        (vmcs::GUEST_CS_ACCESS_RIGHTS, host::seg_access(cs) as u64),
        (vmcs::GUEST_SS_ACCESS_RIGHTS, host::seg_access(ss) as u64),
        (vmcs::GUEST_DS_ACCESS_RIGHTS, host::seg_access(ds) as u64),
        (vmcs::GUEST_FS_ACCESS_RIGHTS, host::seg_access(fs) as u64),
        (vmcs::GUEST_GS_ACCESS_RIGHTS, host::seg_access(gs) as u64),
        (vmcs::GUEST_LDTR_ACCESS_RIGHTS, host::seg_access(ldtr) as u64),
        (vmcs::GUEST_TR_ACCESS_RIGHTS, host::seg_access(g.tr_sel) as u64),
        (vmcs::GUEST_SYSENTER_CS, asm::rdmsr(vmcs::IA32_SYSENTER_CS)),
        (vmcs::GUEST_INTERRUPTIBILITY, 0),
        (vmcs::GUEST_ACTIVITY_STATE, 0),
        (vmcs::GUEST_SMBASE, 0),
        (vmcs::VMX_PREEMPT_TIMER, 0),
        // natural: bases + state
        (
            vmcs::GUEST_CR0,
            host::adjust_guest_cr0(asm::read_cr(0)),
        ),
        (vmcs::GUEST_CR3, asm::read_cr(3)),
        (
            vmcs::GUEST_CR4,
            host::adjust_guest_cr4(asm::read_cr(4)),
        ),
        (vmcs::GUEST_ES_BASE, host::seg_base(gdtr.base, es)),
        (vmcs::GUEST_CS_BASE, host::seg_base(gdtr.base, cs)),
        (vmcs::GUEST_SS_BASE, host::seg_base(gdtr.base, ss)),
        (vmcs::GUEST_DS_BASE, host::seg_base(gdtr.base, ds)),
        (vmcs::GUEST_FS_BASE, asm::rdmsr(vmcs::IA32_FS_BASE)),
        (vmcs::GUEST_GS_BASE, asm::rdmsr(vmcs::IA32_GS_BASE)),
        (vmcs::GUEST_LDTR_BASE, host::seg_base(g.gdt_base, ldtr)),
        (vmcs::GUEST_TR_BASE, host::seg_base(g.gdt_base, g.tr_sel)),
        (vmcs::GUEST_GDTR_BASE, g.gdt_base),
        (vmcs::GUEST_IDTR_BASE, g.gdt_base + 0x1000),
        (vmcs::GUEST_DR7, 0x400),
        (vmcs::GUEST_RSP, g.rsp),
        (vmcs::GUEST_RIP, g.rip),
        (vmcs::GUEST_RFLAGS, 0x202),
        (vmcs::GUEST_PENDING_DEBUG, 0),
        (
            vmcs::GUEST_SYSENTER_ESP,
            asm::rdmsr(vmcs::IA32_SYSENTER_ESP),
        ),
        (
            vmcs::GUEST_SYSENTER_EIP,
            asm::rdmsr(vmcs::IA32_SYSENTER_EIP),
        ),
    ];
    writes.iter().all(|&(f, v)| asm::vmwrite(f, v).is_ok())
}

// The tiny first guest: exercises CPUID exiting, a PING hypercall (which
// must return 0xC0FFEE), then a STOP hypercall that drops the CPU back
// to the host. Written as a global_asm blob so it has no Rust ABI or
// stack requirements beyond the page we give it.
core::arch::global_asm!(
    ".globl uefios_guest_entry",
    "uefios_guest_entry:",
    "mov $0, %eax",
    "xor %ecx, %ecx",
    "cpuid",                                    // VM-exit: CPUID
    "movabs $0x5545464900000000, %rcx",         // 'UEFI'<<32 | 0 = PING
    "vmcall",
    "cmp $0xc0ffee, %rax",
    "jne 1f",
    "movabs $0x5545464900000001, %rcx",         // ... | 1 = STOP
    "vmcall",
    // STOP should not return; HLT loop if it does (HLT exiting is on,
    // so we just bounce back — the report will show the real reason).
    "1: hlt",
    "jmp 1b",
    options(att_syntax),
);

extern "C" {
    /// Guest entry point; pass `uefios_guest_entry as usize as u64` as
    /// [`GuestInit::rip`].
    pub fn uefios_guest_entry();
}

/// Hypercall signature/numbers the exit handler recognizes in RCX.
pub const HC_MAGIC: u64 = 0x5545_4649_0000_0000; // "UEFI"
pub const HC_PING: u64 = 0;
pub const HC_STOP: u64 = 1;
/// Value the handler puts in RAX on PING (guest asserts on it).
pub const HC_PING_REPLY: u64 = 0xc0ffee;
