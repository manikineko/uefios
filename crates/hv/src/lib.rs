#![no_std]

//! Type-1 hypervisor running inside the uefios boot-services environment.
//!
//! This is what the "containerization that runs multiple operating systems at
//! once" actually requires on bare metal: hardware virtualization. The host
//! (uefios) runs in VMX root mode; each guest OS runs non-root in a VMCS with
//! its own EPT. Each guest's virtual display is composited into a `wm` window.
//!
//! Bring-up sequence (mirrors Intel SDM Vol. 3C, chapter 31):
//! 1. CPUID.1:ECX.VMX[5]            — implemented below
//! 2. IA32_FEATURE_CONTROL MSR lock bit + VMX-enable outside SMX
//! 3. CR4.VMXE, allocate VMXON region, `vmxon`
//! 4. Per-VM: allocate VMCS, `vmptrld`, populate guest/host state + controls,
//!    `vmlaunch`
//! 5. EPT for guest physical memory; vmexit handlers for I/O and device
//!    emulation / paravirt display
//!
//! AMD-V (SVM) follows the same structure with VMRUN/VMCB instead.

/// True if the CPU reports Intel VMX support (CPUID.1:ECX bit 5).
///
/// This only proves the silicon has VT-x — the firmware may still have it
/// locked off via `IA32_FEATURE_CONTROL`, which is step 2 of bring-up.
#[cfg(target_arch = "x86_64")]
pub fn vmx_supported() -> bool {
    use core::arch::x86_64::__cpuid;
    __cpuid(1).ecx & (1 << 5) != 0
}

/// True if the CPU reports AMD SVM support (CPUID.80000001h:ECX bit 2).
#[cfg(target_arch = "x86_64")]
pub fn svm_supported() -> bool {
    use core::arch::x86_64::__cpuid;
    __cpuid(0x8000_0001).ecx & (1 << 2) != 0
}

#[cfg(not(target_arch = "x86_64"))]
pub fn vmx_supported() -> bool {
    false
}

#[cfg(not(target_arch = "x86_64"))]
pub fn svm_supported() -> bool {
    false
}
