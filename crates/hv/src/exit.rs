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
