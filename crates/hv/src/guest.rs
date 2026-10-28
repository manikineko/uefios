//! Guest-state VMCS fields + a minimal first-launch context, modeled on
//! MiniVisor's guest init (64-bit guest, real-mode is later).

#![allow(dead_code)]

use crate::asm;
use crate::vmcs;

/// EPT guest-physical view: identity map for the first 4 GiB using
/// 2 MiB superpages (PML4 + PDPT + PD — three 4 KiB pages suffice).
#[repr(C, align(4096))]
pub struct Ept {
    pub pml4: [u64; 512],
    pub pdpt: [u64; 512],
    pub pd: [[u64; 512]; 4],
}

impl Ept {
    /// Fill tables so GPA==HPA, full RWX. `base` is this struct's PA.
    pub unsafe fn init_identity(&mut self, base: u64) {
        let pdpt_pa = base + core::mem::size_of::<[u64; 512]>() as u64;
        self.pml4[0] = pdpt_pa | crate::ept::EPT_READ | crate::ept::EPT_WRITE | crate::ept::EPT_EXEC;
        let pd_base = pdpt_pa + core::mem::size_of::<[u64; 512]>() as u64;
        for i in 0..4 {
            self.pdpt[i] = pd_base + i as u64 * 4096
                | crate::ept::EPT_READ | crate::ept::EPT_WRITE | crate::ept::EPT_EXEC;
            for j in 0..512 {
                let gpa = (i * 512 + j) as u64 * 0x20_0000;
                self.pd[i][j] = crate::ept::superpage(gpa);
            }
        }
    }
}

/// Write the fields a first 64-bit guest needs. Caller owns EPT + fills
/// segment regs; this sets state + execution controls incl. EPTP.
pub unsafe fn write_guest_fields(eptp: u64, guest_rip: u64, guest_rsp: u64) -> bool {
    let ok = asm::vmwrite(vmcs::GUEST_RIP, guest_rip).is_ok()
        && asm::vmwrite(vmcs::GUEST_RSP, guest_rsp).is_ok()
        && asm::vmwrite(vmcs::GUEST_RFLAGS, 0x2).is_ok()
        && asm::vmwrite(vmcs::GUEST_CR0, 0x6000_0010).is_ok() // NE|PG-ish minimal
        && asm::vmwrite(vmcs::GUEST_CR4, 0x2000).is_ok()      // VMXE off in guest
        && asm::vmwrite(vmcs::EPT_POINTER, eptp).is_ok()
        && asm::vmwrite(vmcs::VMCS_LINK_POINTER, 0xffff_ffff_ffff_ffff).is_ok();
    ok
}
