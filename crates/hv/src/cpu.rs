//! Per-CPU VMX bring-up: VMXON region alloc, feature-control check,
//! vmxon/vmptrld/vmlaunch wrappers. Skeleton — asm stubs go in asm.rs.

#![allow(dead_code)]

use crate::asm;
use crate::vmcs;

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
    let fc = asm::rdmsr(vmcs::IA32_FEATURE_CONTROL);
    fc & 1 != 0 && fc & (1 << 2) != 0
}

/// Number of bytes the VMCS requires (IA32_VMX_BASIC bits 44:32, <=4096).
pub fn vmcs_size() -> u64 {
    (asm::rdmsr(vmcs::IA32_VMX_BASIC) >> 32) & 0x1fff
}

/// IA32_VMX_BASIC[30:0] = required VMCS revision id.
pub fn vmcs_revision() -> u32 {
    (asm::rdmsr(vmcs::IA32_VMX_BASIC) as u32) & 0x7fff_ffff
}

/// Set IA32_FEATURE_CONTROL lock + VMXON-enable bit if not already set.
pub fn ensure_feature_control() -> bool {
    let mut fc = asm::rdmsr(vmcs::IA32_FEATURE_CONTROL);
    if fc & 1 == 0 {
        fc |= 1 | (1 << 2);
        asm::wrmsr(vmcs::IA32_FEATURE_CONTROL, fc);
    }
    fc & (1 << 2) != 0
}

/// One-time VMXON for this CPU. `region_pa` must be a 4 KiB-aligned
/// VmxRegion whose revision_id came from IA32_VMX_BASIC.
pub fn vmxon(region_pa: u64) -> bool {
    ensure_feature_control() && unsafe { asm::vmxon(region_pa).is_ok() }
}

/// Load a VMCS region for editing.
pub fn vmcs_load(vmcs_pa: u64) -> bool {
    unsafe { asm::vmclear(vmcs_pa).is_ok() && asm::vmptrld(vmcs_pa).is_ok() }
}
