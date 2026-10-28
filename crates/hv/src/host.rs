//! Host-side VMX bring-up, modeled on MiniVisor's HostInitialization.c:
//! per-CPU VMXON region -> feature control -> VMCS -> host/guest fields.
//!
//! Memory: UEFI runs identity-mapped, so a pool/page VA is also its PA.
//! Caller allocates 4 KiB regions via AllocatePages and hands us VAs.

#![allow(dead_code)]

use crate::asm;
use crate::cpu;
use crate::vmcs;

/// Per-CPU virtualization state (regions must stay 4 KiB-aligned).
#[repr(C, align(4096))]
pub struct VmxCpu {
    /// VMXON region — entered root mode on it.
    pub vmxon: cpu::VmxRegion,
    /// Currently-loaded VMCS for editing/launch.
    pub vmcs: cpu::VmxRegion,
    /// Did vmxon succeed (CPU now in VMX root operation)?
    pub rooted: bool,
}

impl VmxCpu {
    pub const fn new() -> Self {
        Self {
            vmxon: cpu::VmxRegion { revision_id: 0, vmx_abort: 0, data: [0; 4088] },
            vmcs: cpu::VmxRegion { revision_id: 0, vmx_abort: 0, data: [0; 4088] },
            rooted: false,
        }
    }

    /// Fill revision ids from IA32_VMX_BASIC then vmxon.
    /// `self_pa` must be the CPU-visible physical address of `self`.
    pub unsafe fn init_root(&mut self, self_pa: u64) -> bool {
        let rev = vmcs_revision();
        self.vmxon.revision_id = rev;
        self.vmcs.revision_id = rev;
        if !cpu::vmxon(self_pa) {
            return false;
        }
        self.rooted = true;
        // VMCS region lives right after VMXON in this 4 KiB struct.
        let vmcs_pa = self_pa + core::mem::offset_of!(Self, vmcs) as u64;
        cpu::vmcs_load(vmcs_pa)
    }
}

/// IA32_VMX_BASIC[30:0] = required VMCS revision id.
fn vmcs_revision() -> u32 {
    let lo: u32;
    unsafe {
        core::arch::asm!("rdmsr", in("ecx") 0x480u32, out("eax") lo, out("edx") _);
    }
    lo & 0x7fff_ffff
}

/// Capability-filtered control value: applies MSR `cap` (0 = reserved0,
/// 1 = allowed1 / fixed1 pair) to a desired setting.
fn ctrl(desired: u64, cap_msr: u32) -> u64 {
    let cap = unsafe {
        let lo: u32;
        let hi: u32;
        core::arch::asm!("rdmsr", in("ecx") cap_msr, out("eax") lo, out("edx") hi);
        ((hi as u64) << 32) | lo as u64
    };
    (desired | (cap & 0xffff_ffff)) & (cap >> 32)
}

/// Fill the minimum host-state fields for a working VMCS.
/// `host_rip` = asm VM-exit entry stub; `host_rsp` = a dedicated stack.
pub unsafe fn write_host_fields(host_rip: u64, host_rsp: u64) -> bool {
    let cr0 = read_cr(0);
    let cr3 = read_cr(3);
    let cr4 = read_cr(4);
    let ok = asm::vmwrite(vmcs::HOST_CR0, cr0).is_ok()
        && asm::vmwrite(vmcs::HOST_CR3, cr3).is_ok()
        && asm::vmwrite(vmcs::HOST_CR4, cr4).is_ok()
        && asm::vmwrite(vmcs::HOST_RIP, host_rip).is_ok()
        && asm::vmwrite(vmcs::HOST_RSP, host_rsp).is_ok()
        && asm::vmwrite(
            vmcs::PIN_VM_EXEC_CONTROLS,
            ctrl(0, 0x481), // IA32_VMX_PINBASED_CTLS
        )
        .is_ok()
        && asm::vmwrite(
            vmcs::CPU_VM_EXEC_CONTROLS,
            ctrl((1 << 7) | (1 << 24) | (1 << 26), 0x482), // HLT-exiting | use MSR bitmaps | use secondary? -> adjust
        )
        .is_ok()
        && asm::vmwrite(vmcs::VM_EXIT_CONTROLS, ctrl((1 << 9) | (1 << 20), 0x483)).is_ok()
        && asm::vmwrite(vmcs::VM_ENTRY_CONTROLS, ctrl(1 << 9, 0x484)).is_ok();
    ok
}

/// Read a control register (CR0/CR3/CR4) via mov.
unsafe fn read_cr(n: u8) -> u64 {
    let v: u64;
    match n {
        0 => core::arch::asm!("mov {}, cr0", out(reg) v),
        3 => core::arch::asm!("mov {}, cr3", out(reg) v),
        4 => core::arch::asm!("mov {}, cr4", out(reg) v),
        _ => unreachable!(),
    }
    v
}
