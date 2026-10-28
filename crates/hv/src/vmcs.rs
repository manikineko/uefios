//! VMCS field encodings (Intel SDM Vol. 3D, App. B) — the subset
//! uefios initializes, matching MiniVisor's SetupVmcs. Each constant is
//! the 16-bit field encoding used with vmread/vmwrite.

// ---------------- 16-bit fields ----------------
// Control
pub const VIRTUAL_PROCESSOR_ID: u16 = 0x0000;
pub const POSTED_INTR_NOTIFY: u16 = 0x0002;
pub const EPTP_INDEX: u16 = 0x0004;
// Guest state
pub const GUEST_ES_SELECTOR: u16 = 0x0800;
pub const GUEST_CS_SELECTOR: u16 = 0x0802;
pub const GUEST_SS_SELECTOR: u16 = 0x0804;
pub const GUEST_DS_SELECTOR: u16 = 0x0806;
pub const GUEST_FS_SELECTOR: u16 = 0x0808;
pub const GUEST_GS_SELECTOR: u16 = 0x080a;
pub const GUEST_LDTR_SELECTOR: u16 = 0x080c;
pub const GUEST_TR_SELECTOR: u16 = 0x080e;
// Host state
pub const HOST_ES_SELECTOR: u16 = 0x0c00;
pub const HOST_CS_SELECTOR: u16 = 0x0c02;
pub const HOST_SS_SELECTOR: u16 = 0x0c04;
pub const HOST_DS_SELECTOR: u16 = 0x0c06;
pub const HOST_FS_SELECTOR: u16 = 0x0c08;
pub const HOST_GS_SELECTOR: u16 = 0x0c0a;
pub const HOST_TR_SELECTOR: u16 = 0x0c0c;

// ---------------- 64-bit fields ----------------
// Control
pub const IO_BITMAP_A: u16 = 0x2000;
pub const IO_BITMAP_B: u16 = 0x2002;
pub const MSR_BITMAP: u16 = 0x2004;
pub const VM_EXIT_MSR_STORE_ADDR: u16 = 0x2006;
pub const VM_EXIT_MSR_LOAD_ADDR: u16 = 0x2008;
pub const VM_ENTRY_MSR_LOAD_ADDR: u16 = 0x200a;
pub const EXECUTIVE_VMCS_PTR: u16 = 0x200c;
pub const TSC_OFFSET: u16 = 0x2010;
pub const VIRT_APIC_PAGE: u16 = 0x2012;
pub const APIC_ACCESS_ADDR: u16 = 0x2014;
pub const POSTED_INTR_DESC: u16 = 0x2016;
pub const VMFUNC_CONTROLS: u16 = 0x2018;
pub const EPT_POINTER: u16 = 0x201a;
pub const EPTP_LIST_ADDR: u16 = 0x2024;
// Guest state — read-only natural/64-bit info lives further below.
pub const GUEST_PHYSICAL_ADDRESS: u16 = 0x2400;
pub const VMCS_LINK_POINTER: u16 = 0x2800;
pub const GUEST_DEBUGCTL: u16 = 0x2802;
pub const GUEST_PAT: u16 = 0x2804;
pub const GUEST_EFER: u16 = 0x2806;
pub const GUEST_PERF_GLOBAL_CTRL: u16 = 0x2808;
pub const GUEST_PDPTE0: u16 = 0x280a;
pub const GUEST_BNDCFGS: u16 = 0x2812;
// Host state
pub const HOST_PAT: u16 = 0x2c00;
pub const HOST_EFER: u16 = 0x2c02;
pub const HOST_PERF_GLOBAL_CTRL: u16 = 0x2c04;

// ---------------- 32-bit fields ----------------
// Control
pub const PIN_VM_EXEC_CONTROLS: u16 = 0x4000;
pub const CPU_VM_EXEC_CONTROLS: u16 = 0x4002;
pub const EXCEPTION_BITMAP: u16 = 0x4004;
pub const PAGE_FAULT_ERROR_MASK: u16 = 0x4006;
pub const PAGE_FAULT_ERROR_MATCH: u16 = 0x4008;
pub const CR3_TARGET_COUNT: u16 = 0x400a;
pub const VM_EXIT_CONTROLS: u16 = 0x400c;
pub const VM_EXIT_MSR_STORE_COUNT: u16 = 0x400e;
pub const VM_EXIT_MSR_LOAD_COUNT: u16 = 0x4010;
pub const VM_ENTRY_CONTROLS: u16 = 0x4012;
pub const VM_ENTRY_MSR_LOAD_COUNT: u16 = 0x4014;
pub const VM_ENTRY_INTR_INFO: u16 = 0x4016;
pub const VM_ENTRY_EXCEPTION_ERROR: u16 = 0x4018;
pub const VM_ENTRY_INSTRUCTION_LEN: u16 = 0x401a;
pub const TPR_THRESHOLD: u16 = 0x401c;
pub const SECONDARY_VM_EXEC: u16 = 0x401e;
// Read-only data
pub const VM_INSTRUCTION_ERROR: u16 = 0x4400;
pub const EXIT_REASON: u16 = 0x4402;
pub const VM_EXIT_INTERRUPTION_INFO: u16 = 0x4404;
pub const VM_EXIT_INTERRUPTION_ERROR: u16 = 0x4406;
pub const IDT_VECTORING_INFO: u16 = 0x4408;
pub const IDT_VECTORING_ERROR: u16 = 0x440a;
pub const VM_EXIT_INSTRUCTION_LEN: u16 = 0x440c;
pub const VM_EXIT_INSTRUCTION_INFO: u16 = 0x440e;
// Guest state
pub const GUEST_ES_LIMIT: u16 = 0x4800;
pub const GUEST_CS_LIMIT: u16 = 0x4802;
pub const GUEST_SS_LIMIT: u16 = 0x4804;
pub const GUEST_DS_LIMIT: u16 = 0x4806;
pub const GUEST_FS_LIMIT: u16 = 0x4808;
pub const GUEST_GS_LIMIT: u16 = 0x480a;
pub const GUEST_LDTR_LIMIT: u16 = 0x480c;
pub const GUEST_TR_LIMIT: u16 = 0x480e;
pub const GUEST_GDTR_LIMIT: u16 = 0x4810;
pub const GUEST_IDTR_LIMIT: u16 = 0x4812;
pub const GUEST_ES_ACCESS_RIGHTS: u16 = 0x4814;
pub const GUEST_CS_ACCESS_RIGHTS: u16 = 0x4816;
pub const GUEST_SS_ACCESS_RIGHTS: u16 = 0x4818;
pub const GUEST_DS_ACCESS_RIGHTS: u16 = 0x481a;
pub const GUEST_FS_ACCESS_RIGHTS: u16 = 0x481c;
pub const GUEST_GS_ACCESS_RIGHTS: u16 = 0x481e;
pub const GUEST_LDTR_ACCESS_RIGHTS: u16 = 0x4820;
pub const GUEST_TR_ACCESS_RIGHTS: u16 = 0x4822;
pub const GUEST_INTERRUPTIBILITY: u16 = 0x4824;
pub const GUEST_ACTIVITY_STATE: u16 = 0x4826;
pub const GUEST_SMBASE: u16 = 0x4828;
pub const GUEST_SYSENTER_CS: u16 = 0x482a;
pub const VMX_PREEMPT_TIMER: u16 = 0x482e;
// Host state
pub const HOST_SYSENTER_CS: u16 = 0x4c00;

// ---------------- natural-width fields ----------------
// Control
pub const CR0_GUEST_HOST_MASK: u16 = 0x6000;
pub const CR4_GUEST_HOST_MASK: u16 = 0x6002;
pub const CR0_READ_SHADOW: u16 = 0x6004;
pub const CR4_READ_SHADOW: u16 = 0x6006;
pub const CR3_TARGET_VALUE0: u16 = 0x6008;
// Read-only data
pub const EXIT_QUALIFICATION: u16 = 0x6400;
pub const GUEST_LINEAR_ADDRESS: u16 = 0x640a;
// Guest state
pub const GUEST_CR0: u16 = 0x6800;
pub const GUEST_CR3: u16 = 0x6802;
pub const GUEST_CR4: u16 = 0x6804;
pub const GUEST_ES_BASE: u16 = 0x6806;
pub const GUEST_CS_BASE: u16 = 0x6808;
pub const GUEST_SS_BASE: u16 = 0x680a;
pub const GUEST_DS_BASE: u16 = 0x680c;
pub const GUEST_FS_BASE: u16 = 0x680e;
pub const GUEST_GS_BASE: u16 = 0x6810;
pub const GUEST_LDTR_BASE: u16 = 0x6812;
pub const GUEST_TR_BASE: u16 = 0x6814;
pub const GUEST_GDTR_BASE: u16 = 0x6816;
pub const GUEST_IDTR_BASE: u16 = 0x6818;
pub const GUEST_DR7: u16 = 0x681a;
pub const GUEST_RSP: u16 = 0x681c;
pub const GUEST_RIP: u16 = 0x681e;
pub const GUEST_RFLAGS: u16 = 0x6820;
pub const GUEST_PENDING_DEBUG: u16 = 0x6822;
pub const GUEST_SYSENTER_ESP: u16 = 0x6824;
pub const GUEST_SYSENTER_EIP: u16 = 0x6826;
// Host state
pub const HOST_CR0: u16 = 0x6c00;
pub const HOST_CR3: u16 = 0x6c02;
pub const HOST_CR4: u16 = 0x6c04;
pub const HOST_FS_BASE: u16 = 0x6c06;
pub const HOST_GS_BASE: u16 = 0x6c08;
pub const HOST_TR_BASE: u16 = 0x6c0a;
pub const HOST_GDTR_BASE: u16 = 0x6c0c;
pub const HOST_IDTR_BASE: u16 = 0x6c0e;
pub const HOST_SYSENTER_ESP: u16 = 0x6c10;
pub const HOST_SYSENTER_EIP: u16 = 0x6c12;
pub const HOST_RSP: u16 = 0x6c14;
pub const HOST_RIP: u16 = 0x6c16;

// ---------------- MSRs ----------------
pub const IA32_SYSENTER_CS: u32 = 0x174;
pub const IA32_SYSENTER_ESP: u32 = 0x175;
pub const IA32_SYSENTER_EIP: u32 = 0x176;
pub const IA32_PAT: u32 = 0x277;
pub const IA32_FEATURE_CONTROL: u32 = 0x3a;
pub const IA32_BIOS_SIGN_ID: u32 = 0x8b;
pub const IA32_VMX_BASIC: u32 = 0x480;
pub const IA32_VMX_PINBASED_CTLS: u32 = 0x481;
pub const IA32_VMX_PROCBASED_CTLS: u32 = 0x482;
pub const IA32_VMX_EXIT_CTLS: u32 = 0x483;
pub const IA32_VMX_ENTRY_CTLS: u32 = 0x484;
pub const IA32_VMX_MISC: u32 = 0x485;
pub const IA32_VMX_CR0_FIXED0: u32 = 0x486;
pub const IA32_VMX_CR0_FIXED1: u32 = 0x487;
pub const IA32_VMX_CR4_FIXED0: u32 = 0x488;
pub const IA32_VMX_CR4_FIXED1: u32 = 0x489;
pub const IA32_VMX_PROCBASED_CTLS2: u32 = 0x48b;
pub const IA32_VMX_TRUE_PINBASED_CTLS: u32 = 0x48d;
pub const IA32_VMX_TRUE_PROCBASED_CTLS: u32 = 0x48e;
pub const IA32_VMX_TRUE_EXIT_CTLS: u32 = 0x48f;
pub const IA32_VMX_TRUE_ENTRY_CTLS: u32 = 0x490;
pub const IA32_VMX_VMFUNC: u32 = 0x491;
pub const IA32_EFER: u32 = 0xc000_0080;
pub const IA32_FS_BASE: u32 = 0xc000_0100;
pub const IA32_GS_BASE: u32 = 0xc000_0101;
pub const IA32_PERF_GLOBAL_CTRL: u32 = 0xc000_038f;

// ---------------- control bits ----------------
// Pin-based (MSR 0x481 / true 0x48d)
pub const PIN_NMI_EXITING: u64 = 1 << 0;
pub const PIN_VIRTUAL_NMIS: u64 = 1 << 5;
// Primary processor-based (MSR 0x482 / true 0x48e)
pub const CPU_HLT_EXITING: u64 = 1 << 7;
pub const CPU_CR3_LOAD_EXITING: u64 = 1 << 15;
pub const CPU_CR3_STORE_EXITING: u64 = 1 << 16;
pub const CPU_CR8_LOAD_EXITING: u64 = 1 << 19;
pub const CPU_CR8_STORE_EXITING: u64 = 1 << 20;
pub const CPU_NMI_WINDOW_EXITING: u64 = 1 << 22;
pub const CPU_MOV_DR_EXITING: u64 = 1 << 23;
pub const CPU_MSR_BITMAPS: u64 = 1 << 28;
pub const CPU_SECONDARY: u64 = 1 << 31;
// Secondary processor-based (MSR 0x48b)
pub const SEC_ENABLE_EPT: u64 = 1 << 1;
pub const SEC_ENABLE_RDTSCP: u64 = 1 << 3;
pub const SEC_ENABLE_VPID: u64 = 1 << 5;
pub const SEC_UNRESTRICTED_GUEST: u64 = 1 << 7;
pub const SEC_ENABLE_INVPCID: u64 = 1 << 12;
pub const SEC_ENABLE_XSAVES: u64 = 1 << 20;
// VM-exit controls (MSR 0x483 / true 0x48f)
pub const EXIT_HOST_ADDR_SPACE_SIZE: u64 = 1 << 9;
pub const EXIT_ACK_INTR: u64 = 1 << 15;
pub const EXIT_SAVE_IA32_EFER: u64 = 1 << 20;
pub const EXIT_LOAD_IA32_EFER: u64 = 1 << 21;
// VM-entry controls (MSR 0x484 / true 0x490)
pub const ENTRY_IA32E_GUEST: u64 = 1 << 9;
pub const ENTRY_LOAD_DEBUG_CONTROLS: u64 = 1 << 13;
pub const ENTRY_LOAD_IA32_EFER: u64 = 1 << 15;

// ---------------- CR bits / fixed MSRs ----------------
pub const CR0_PE: u64 = 1 << 0;
pub const CR0_NE: u64 = 1 << 5;
pub const CR0_PG: u64 = 1 << 31;
pub const CR4_VMXE: u64 = 1 << 13;

// ---------------- access-rights helpers ----------------
/// Access-rights bit: segment is unusable (null selector / lar failed).
pub const ACCESS_UNUSABLE: u32 = 1 << 16;

/// Full access-rights dword from raw `lar` output. LAR places the
/// descriptor access byte at bits 15:8 and the flags nibble (AVL/L/D-B/G)
/// at 23:20; the VMCS format wants them at 11:0 and 15:12 — i.e. `lar >> 8`
/// with bits 11:8 (limit[19:16] garbage) and 31:16 cleared. Same transform
/// as MiniVisor's GetSegmentAccessRight.
pub const fn access_from_lar(lar: u64, unusable: bool) -> u32 {
    let mut a = ((lar >> 8) & 0xf0ff) as u32;
    if unusable {
        a |= ACCESS_UNUSABLE;
    }
    a
}
