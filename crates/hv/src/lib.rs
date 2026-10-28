//! hv — type-1 hypervisor on Intel VT-x (AMD-V later), modeled on the
//! MIT-licensed MiniVisor (tandasat/MiniVisorPkg, reference vendored at
//! userland/minivisor-ref). Runs as part of uefios without ever calling
//! ExitBootServices: VMXON makes us root, boot services keep running as
//! the host's "kernel".
//!
//! ## Bring-up order (mirrors MiniVisor's design)
//!
//! 1. **Detect**: CPUID.1:ECX.VMX + IA32_FEATURE_CONTROL MSR lock bit.
//! 2. **VMXON region**: 4 KiB page-aligned region per CPU, revision-id
//!    from IA32_VMX_BASIC MSR (bits 30:0), `vmxon [addr]`.
//! 3. **EPT**: identity-map guest physical -> host physical for RAM;
//!    EPT page-table type 6 (WB), walk length 4. Needed so the guest can
//!    run with its own address space while we intercept.
//! 4. **VMCS per logical processor**: `vmclear` + `vmptrld`, then fill
//!    guest/host fields (see `vmcs` module for field encodings).
//! 5. **vmlaunch**: guest picks up the to-be-virtualized context;
//!    VMExits land in our handler via host RIP.
//! 6. **Guests**: uefios framebuffer windows == guest displays; I/O to
//!    real hardware is either passed through (EPT/MMIO traps) or
//!    paravirtualized via our console protocol.
//!
//! Intel SDM Vol. 3C is authoritative; file names/comments follow
//! MiniVisor's structure so the vendored C sources map 1:1.

#![no_std]
extern crate alloc;

pub mod vmcs;
pub mod ept;
pub mod cpu;

/// CPU virtualization features as reported by CPUID/MSRs.
pub struct Caps {
    pub vmx: bool,
    pub svm: bool,
}

/// Probe the platform once at boot.
pub fn probe() -> Caps {
    let ecx = core::arch::x86_64::__cpuid(1).ecx;
    let edx = core::arch::x86_64::__cpuid(0x8000_0001).edx;
    Caps { vmx: ecx & (1 << 5) != 0, svm: edx & (1 << 2) != 0 }
}
