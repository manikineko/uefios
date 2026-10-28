//! Per-CPU VMX bring-up: VMXON region alloc, feature-control check,
//! vmxon/vmptrld/vmlaunch wrappers. Skeleton — asm stubs go in asm.rs.

#![allow(dead_code)]

/// 4 KiB-aligned VMXON/VMCS region (one per logical CPU).
#[repr(C, align(4096))]
pub struct VmxRegion {
    pub revision_id: u32,
    pub vmx_abort: u32,
    pub data: [u8; 4096 - 8],
}

/// Check IA32_FEATURE_CONTROL (0x3a): bit0 lock, bit2 VMXON-outside-SMX.
/// If unlocked we set it ourselves before vmxon.
pub fn feature_control_ok() -> bool {
    let fc = unsafe {
        let lo: u32;
        let hi: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0x3au32,
            out("eax") lo, out("edx") hi,
        );
        ((hi as u64) << 32) | lo as u64
    };
    fc & 1 != 0 && fc & (1 << 2) != 0
}

/// Number of bytes the VMCS requires (IA32_VMX_BASIC bits 44:32, <=4096).
pub fn vmcs_size() -> u64 {
    let basic = unsafe {
        let lo: u32;
        let hi: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0x480u32,
            out("eax") lo, out("edx") hi,
        );
        ((hi as u64) << 32) | lo as u64
    };
    (basic >> 32) & 0x1fff
}
