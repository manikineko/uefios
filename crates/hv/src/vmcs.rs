//! VMCS field encodings (Intel SDM Vol. 3D, App. B) — the subset
//! MiniVisor initializes. Each constant is the 16-bit field encoding
//! used with vmread/vmwrite.

// 16-bit control fields
pub const VIRTUAL_PROCESSOR_ID: u16 = 0x0000;
pub const POSTED_INTR_NOTIFY: u16 = 0x0002;
pub const EPTP_INDEX: u16 = 0x0004;
pub const GUEST_ES_SELECTOR: u16 = 0x0800;
pub const HOST_ES_SELECTOR: u16 = 0x0c00;

// 32-bit control fields
pub const PIN_VM_EXEC_CONTROLS: u16 = 0x4000;
pub const CPU_VM_EXEC_CONTROLS: u16 = 0x4002;
pub const EXCEPTION_BITMAP: u16 = 0x4004;
pub const VM_EXIT_CONTROLS: u16 = 0x400c;
pub const VM_ENTRY_CONTROLS: u16 = 0x4012;
pub const SECONDARY_VM_EXEC: u16 = 0x401e;

// Natural-width guest/host state
pub const VM_EXIT_MSR_LOAD_COUNT: u16 = 0x200e;
pub const VM_ENTRY_MSR_LOAD_COUNT: u16 = 0x200a;
pub const GUEST_CR0: u16 = 0x6800;
pub const GUEST_CR3: u16 = 0x6802;
pub const GUEST_CR4: u16 = 0x6804;
pub const GUEST_RIP: u16 = 0x681e;
pub const GUEST_RSP: u16 = 0x681c;
pub const GUEST_RFLAGS: u16 = 0x6820;
pub const HOST_CR0: u16 = 0x6c00;
pub const HOST_CR3: u16 = 0x6c02;
pub const HOST_CR4: u16 = 0x6c04;
pub const HOST_RIP: u16 = 0x6c16;
pub const HOST_RSP: u16 = 0x6c14;

// 64-bit fields
pub const EPT_POINTER: u16 = 0x201a;
pub const MSR_BITMAP: u16 = 0x2004;
pub const VMCS_LINK_POINTER: u16 = 0x2800;

/// VMCS revision-id source: IA32_VMX_BASIC (0x480), bits 30:0.
pub const IA32_VMX_BASIC: u32 = 0x480;
/// Feature-control lock/enable MSR (0x3a).
pub const IA32_FEATURE_CONTROL: u32 = 0x3a;
