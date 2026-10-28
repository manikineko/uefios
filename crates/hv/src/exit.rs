//! VM-exit handling: the host RIP the VMCS points at on every exit.
//! An outer asm stub saves GPRs + captures the guest state, then calls
//! `handle_vmexit`. Design mirrors MiniVisor's AsmHostEntrypoint.

#![allow(dead_code)]

/// Reason codes for the most common VM-exits (VMCS 0x4402, exit reason).
pub const EXIT_EXCEPTION_NMI: u64 = 0;
pub const EXIT_CPUID: u64 = 10;
pub const EXIT_HLT: u64 = 12;
pub const EXIT_VMCALL: u64 = 18;
pub const EXIT_MSR_READ: u64 = 31;
pub const EXIT_MSR_WRITE: u64 = 32;
pub const EXIT_EPT_VIOLATION: u64 = 48;
pub const EXIT_EPT_MISCONFIG: u64 = 49;

/// Minimal handler: reads the exit reason + qualification and decides
/// whether to resume the guest or stay in host. Real logic comes later —
/// this is where device emulation / dispatch hooks land.
#[no_mangle]
pub extern "C" fn handle_vmexit(guest_regs: *mut u64) -> i32 {
    let reason = unsafe { crate::asm::vmread(0x4402).unwrap_or(u64::MAX) } & 0xffff;
    let _qual = unsafe { crate::asm::vmread(0x6400).unwrap_or(0) };
    // Default policy: resume the guest for now; -1 would mean "stay out".
    let _ = (reason, guest_regs);
    0
}

// Host entrypoint installed in HOST_RIP. On VMExit the CPU swaps to host
// state; this stub pushes the guest's GPRs, calls handle_vmexit, then
// restores and vmresumes. Mirrors MiniVisor's AsmHostEntrypoint.
core::arch::global_asm!(
    ".global uefios_vmexit_entry",
    "uefios_vmexit_entry:",
    // Save all 15 GPRs (RIP is in the VMCS, not on stack).
    "pushq %rax", "pushq %rcx", "pushq %rdx", "pushq %rbx", "pushq %rbp",
    "pushq %rsi", "pushq %rdi", "pushq %r8", "pushq %r9", "pushq %r10",
    "pushq %r11", "pushq %r12", "pushq %r13", "pushq %r14", "pushq %r15",
    "movq %rsp, %rdi",              // arg1 = &guest_regs
    "subq $0x20, %rsp",
    "call handle_vmexit",
    "addq $0x20, %rsp",
    // Restore and resume the guest.
    "popq %r15", "popq %r14", "popq %r13", "popq %r12", "popq %r11",
    "popq %r10", "popq %r9", "popq %r8", "popq %rdi", "popq %rsi",
    "popq %rbp", "popq %rbx", "popq %rdx", "popq %rcx", "popq %rax",
    "vmresume",
    "ud2",                          // unreachable if vmresume faults
    options(att_syntax),
);
extern "C" {
    /// Symbol for the VMExit entrypoint; pass `as usize as u64` as HOST_RIP.
    pub fn uefios_vmexit_entry();
}
