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

    /// Fill the VMXON revision id then vmxon + vmclear + vmptrld.
    /// `self_pa` must be the CPU-visible physical address of `self`.
    /// The VMCS page is zeroed before its revision id is written:
    /// post-VMCLEAR field contents are undefined, so every field we
    /// rely on gets an explicit vmwrite.
    pub unsafe fn init_root(&mut self, self_pa: u64) -> bool {
        let rev = cpu::vmcs_revision();
        self.vmxon.revision_id = rev;
        if !cpu::vmxon(self_pa) {
            return false;
        }
        self.rooted = true; // vmxon succeeded — we're in VMX root mode
        let vmcs_pa = self_pa + core::mem::offset_of!(Self, vmcs) as u64;
        core::ptr::write_bytes(&mut self.vmcs as *mut _ as *mut u8, 0, 4096);
        self.vmcs.revision_id = rev;
        cpu::vmcs_load(vmcs_pa)
    }

    /// Are we in VMX root operation (post-vmxon)?
    pub fn is_rooted(&self) -> bool {
        self.rooted
    }
}

// ---------------- capability-adjusted control values ----------------

/// True-capability MSRs exist when IA32_VMX_BASIC bit 55 is set; they
/// replace the legacy capability MSRs for pin/proc/exit/entry controls.
fn cap_msr(legacy: u32, true_msr: u32) -> u32 {
    if asm::rdmsr(vmcs::IA32_VMX_BASIC) & (1 << 55) != 0 {
        true_msr
    } else {
        legacy
    }
}

/// Capability-filtered control value: applies MSR `cap` (bits 31:0 =
/// allowed-0/reserved, bits 63:32 = allowed-1) to a desired setting.
fn ctrl(desired: u64, cap_msr: u32) -> u64 {
    let cap = asm::rdmsr(cap_msr);
    (desired | (cap & 0xffff_ffff)) & (cap >> 32)
}

/// Apply CR0 fixed0/fixed1 so a guest CR0 value is VMX-legal.
pub fn adjust_guest_cr0(v: u64) -> u64 {
    (v | asm::rdmsr(vmcs::IA32_VMX_CR0_FIXED0)) & asm::rdmsr(vmcs::IA32_VMX_CR0_FIXED1)
}

/// Apply CR4 fixed0/fixed1 (note VMXE is fixed-1 on every VT-x part).
pub fn adjust_guest_cr4(v: u64) -> u64 {
    (v | asm::rdmsr(vmcs::IA32_VMX_CR4_FIXED0)) & asm::rdmsr(vmcs::IA32_VMX_CR4_FIXED1)
}

// ---------------- host descriptor tables ----------------

/// Two pages the caller supplies for our private GDT + TSS + IDT:
///   [0x000..0x800) new GDT (old GDT copied + one 16-byte TSS desc)
///   [0x800..0x900) 64-bit TSS (104 bytes)
///   [0x1000..0x2000) copy of the IDT, for the guest's IDTR — the
///                    originals can live above the EPT map.
/// Returns the selector of the new TR, or 0 on failure.
pub unsafe fn install_tables(page: *mut u8) -> u16 {
    let cur = asm::sgdt();
    let old_len = (cur.limit as usize) + 1;
    if old_len + 16 > 0x800 {
        return 0;
    }
    // Clone the live GDT.
    core::ptr::copy_nonoverlapping(cur.base as *const u8, page, old_len);

    // TSS descriptor for a 64-bit TSS at page+0x800.
    let tss_pa = page as u64 + 0x800;
    let tss = page.add(0x800);
    core::ptr::write_bytes(tss, 0, 0x68); // 104 bytes

    let idx = old_len / 8;
    let d = page.add(old_len) as *mut u64;
    // Low qword: limit 0x67, type 9 (available 64-bit TSS), present,
    // base split across descriptor base fields (SDM 3.4.5.1).
    let lo = 0x67u64
        | ((tss_pa & 0xff_ffff) << 16) // base[23:0]
        | (0x9 << 40) // type
        | (1 << 47) // present
        | ((tss_pa >> 24) & 0xff) << 56; // base[31:24]
    d.write(lo);
    d.add(1).write(tss_pa >> 32); // base[63:32]

    // Clone the IDT for the guest's IDTR (host keeps its own).
    let idtr = asm::sidt();
    let idt_len = (idtr.limit as usize) + 1;
    if idt_len > 0x1000 {
        return 0;
    }
    core::ptr::copy_nonoverlapping(idtr.base as *const u8, page.add(0x1000), idt_len);

    let sel = (idx << 3) as u16;
    asm::lgdt(page as u64, (old_len + 16 - 1) as u16);
    asm::write_tr(sel);
    sel
}

/// Segment-base from a descriptor in `gdt_base` for selector `sel`.
/// Follows MiniVisor's GetSegmentBase: null/TI selectors report 0.
pub unsafe fn seg_base(gdt_base: u64, sel: u16) -> u64 {
    if sel == 0 || sel & 4 != 0 {
        return 0;
    }
    let d = (gdt_base + (sel & 0xfff8) as u64) as *const u64;
    let lo = *d;
    let mut base = (lo >> 16) & 0xff_ffff;
    base |= ((lo >> 56) & 0xff) << 24;
    // Only system descriptors (S=0, e.g. TSS/LDT) have a 64-bit base
    // with an upper dword; don't read past a code/data descriptor.
    if lo & (1 << 44) == 0 {
        base |= *d.add(1) << 32;
    }
    base
}

/// Limit via lsl; 0xFFFF when the selector is null/unusable.
pub fn seg_limit(sel: u16) -> u64 {
    if sel == 0 {
        return 0xffff;
    }
    asm::lsl(sel).unwrap_or(0xffff)
}

/// Access-rights dword via lar; `ACCESS_UNUSABLE` when the selector
/// cannot be loaded (e.g. a null LDTR, which the SDM permits).
pub fn seg_access(sel: u16) -> u32 {
    if sel == 0 {
        return vmcs::ACCESS_UNUSABLE;
    }
    match asm::lar(sel) {
        Some(a) => vmcs::access_from_lar(a, false),
        None => vmcs::ACCESS_UNUSABLE,
    }
}

// ---------------- host-state + control fields ----------------

/// Fill host-state fields. `host_rip` = asm VM-exit entry stub,
/// `host_rsp` = dedicated hypervisor stack, `tr_sel` = selector from
/// [`install_tables`], `gdt_base` = our private GDT base.
pub unsafe fn write_host_fields(
    tr_sel: u16,
    gdt_base: u64,
    host_rip: u64,
    host_rsp: u64,
) -> bool {
    // "The RPL (bits 1:0) and the TI flag (bit 2) must be 0" (26.2.3).
    let mask = !7u64;
    let idtr = asm::sidt();
    let writes = [
        (vmcs::HOST_ES_SELECTOR, asm::read_es() as u64 & mask),
        (vmcs::HOST_CS_SELECTOR, asm::read_cs() as u64 & mask),
        (vmcs::HOST_SS_SELECTOR, asm::read_ss() as u64 & mask),
        (vmcs::HOST_DS_SELECTOR, asm::read_ds() as u64 & mask),
        (vmcs::HOST_FS_SELECTOR, asm::read_fs() as u64 & mask),
        (vmcs::HOST_GS_SELECTOR, asm::read_gs() as u64 & mask),
        (vmcs::HOST_TR_SELECTOR, tr_sel as u64 & mask),
        (vmcs::HOST_EFER, asm::rdmsr(vmcs::IA32_EFER)),
        (vmcs::HOST_PAT, asm::rdmsr(vmcs::IA32_PAT)),
        (vmcs::HOST_PERF_GLOBAL_CTRL, asm::rdmsr(vmcs::IA32_PERF_GLOBAL_CTRL)),
        (vmcs::HOST_SYSENTER_CS, asm::rdmsr(vmcs::IA32_SYSENTER_CS)),
        (vmcs::HOST_CR0, asm::read_cr(0)),
        (vmcs::HOST_CR3, asm::read_cr(3)),
        (vmcs::HOST_CR4, asm::read_cr(4)),
        (vmcs::HOST_FS_BASE, asm::rdmsr(vmcs::IA32_FS_BASE)),
        (vmcs::HOST_GS_BASE, asm::rdmsr(vmcs::IA32_GS_BASE)),
        (vmcs::HOST_TR_BASE, seg_base(gdt_base, tr_sel)),
        (vmcs::HOST_GDTR_BASE, gdt_base),
        (vmcs::HOST_IDTR_BASE, idtr.base),
        (vmcs::HOST_SYSENTER_ESP, asm::rdmsr(vmcs::IA32_SYSENTER_ESP)),
        (vmcs::HOST_SYSENTER_EIP, asm::rdmsr(vmcs::IA32_SYSENTER_EIP)),
        (vmcs::HOST_RSP, host_rsp),
        (vmcs::HOST_RIP, host_rip),
    ];
    writes.iter().all(|&(f, v)| asm::vmwrite(f, v).is_ok())
}

/// Fill all VM-execution/exit/entry control fields. `eptp` is the EPTP
/// value from [`crate::ept::eptp`]; `msr_bitmap_pa` is a zeroed 4 KiB
/// page (see [`init_msr_bitmap`]); `guest_cr0`/`guest_cr4` are the
/// *unadjusted* guest values mirrored into the CRn read shadows.
pub unsafe fn write_controls(
    eptp: u64,
    msr_bitmap_pa: u64,
    guest_cr0: u64,
    guest_cr4: u64,
) -> bool {
    let pin = ctrl(
        vmcs::PIN_NMI_EXITING | vmcs::PIN_VIRTUAL_NMIS,
        cap_msr(vmcs::IA32_VMX_PINBASED_CTLS, vmcs::IA32_VMX_TRUE_PINBASED_CTLS),
    );
    let primary = ctrl(
        vmcs::CPU_HLT_EXITING | vmcs::CPU_MSR_BITMAPS | vmcs::CPU_SECONDARY,
        cap_msr(
            vmcs::IA32_VMX_PROCBASED_CTLS,
            vmcs::IA32_VMX_TRUE_PROCBASED_CTLS,
        ),
    );
    let secondary = ctrl(
        vmcs::SEC_ENABLE_EPT
            | vmcs::SEC_ENABLE_VPID
            | vmcs::SEC_ENABLE_RDTSCP
            | vmcs::SEC_UNRESTRICTED_GUEST
            | vmcs::SEC_ENABLE_INVPCID
            | vmcs::SEC_ENABLE_XSAVES,
        vmcs::IA32_VMX_PROCBASED_CTLS2,
    );
    let exit = ctrl(
        vmcs::EXIT_HOST_ADDR_SPACE_SIZE
            | vmcs::EXIT_SAVE_IA32_EFER
            | vmcs::EXIT_LOAD_IA32_EFER,
        cap_msr(vmcs::IA32_VMX_EXIT_CTLS, vmcs::IA32_VMX_TRUE_EXIT_CTLS),
    );
    let entry = ctrl(
        vmcs::ENTRY_IA32E_GUEST | vmcs::ENTRY_LOAD_IA32_EFER,
        cap_msr(vmcs::IA32_VMX_ENTRY_CTLS, vmcs::IA32_VMX_TRUE_ENTRY_CTLS),
    );

    let writes = [
        (vmcs::VIRTUAL_PROCESSOR_ID, 1),
        (vmcs::MSR_BITMAP, msr_bitmap_pa),
        (vmcs::EPT_POINTER, eptp),
        (vmcs::EXCEPTION_BITMAP, 0),
        (vmcs::PAGE_FAULT_ERROR_MASK, 0),
        (vmcs::PAGE_FAULT_ERROR_MATCH, 0),
        (vmcs::CR3_TARGET_COUNT, 0),
        (vmcs::TPR_THRESHOLD, 0),
        (vmcs::VM_EXIT_MSR_STORE_COUNT, 0),
        (vmcs::VM_EXIT_MSR_LOAD_COUNT, 0),
        (vmcs::VM_ENTRY_MSR_LOAD_COUNT, 0),
        (vmcs::VM_ENTRY_INTR_INFO, 0),
        (vmcs::PIN_VM_EXEC_CONTROLS, pin),
        (vmcs::CPU_VM_EXEC_CONTROLS, primary),
        (vmcs::SECONDARY_VM_EXEC, secondary),
        (vmcs::VM_EXIT_CONTROLS, exit),
        (vmcs::VM_ENTRY_CONTROLS, entry),
        // Intercept guest writes to PG|NE and VMXE: it keeps its view in
        // the read shadows while the real values stay VMX-legal.
        (vmcs::CR0_GUEST_HOST_MASK, vmcs::CR0_NE | vmcs::CR0_PG),
        (vmcs::CR0_READ_SHADOW, guest_cr0),
        (vmcs::CR4_GUEST_HOST_MASK, vmcs::CR4_VMXE),
        (vmcs::CR4_READ_SHADOW, guest_cr4),
    ];
    writes.iter().all(|&(f, v)| asm::vmwrite(f, v).is_ok())
}

/// MSR bitmap page layout (SDM 24.6.9): read-low, read-high,
/// write-low, write-high; "high" covers MSRs 0xC0000000-0xC0001FFF.
/// Zeroed = no exits except the MSRs we explicitly intercept — we trap
/// reads of IA32_BIOS_SIGN_ID like MiniVisor (blocks guest microcode
/// update attempts).
pub unsafe fn init_msr_bitmap(page: *mut u8) {
    core::ptr::write_bytes(page, 0, 4096);
    let read_low = page;
    *(read_low.add(vmcs::IA32_BIOS_SIGN_ID as usize / 8)) |=
        1 << (vmcs::IA32_BIOS_SIGN_ID % 8);
}
