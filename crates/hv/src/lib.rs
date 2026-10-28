//! hv — type-1 hypervisor on Intel VT-x (AMD-V later), modeled on the
//! MIT-licensed MiniVisor (tandasat/MiniVisorPkg, reference vendored at
//! userland/minivisor-ref). Runs as part of uefios without ever calling
//! ExitBootServices: VMXON makes us root, boot services keep running as
//! the host's "kernel".
//!
//! ## Bring-up order (mirrors MiniVisor's design)
//!
//! 1. **Detect**: CPUID.1:ECX.VMX + IA32_FEATURE_CONTROL MSR lock bit.
//! 2. **Descriptor tables**: clone the GDT + install a TSS — both guest
//!    and host TR must name a real TSS descriptor (SDM 26.2.3/26.3.1.2),
//!    which UEFI usually lacks. See [`host::install_tables`].
//! 3. **VMXON region**: 4 KiB page-aligned region per CPU, revision-id
//!    from IA32_VMX_BASIC MSR (bits 30:0), `vmxon [addr]`.
//! 4. **VMCS**: zero + `vmclear` + `vmptrld`, then fill host state
//!    (current CPU clone), control fields (EPT, MSR bitmaps, NMI
//!    exiting, CR0/CR4 shadows), and guest state (CPU clone + our own
//!    RIP/RSP). See [`host`] / [`guest`].
//! 5. **vmlaunch**: via [`launch`]. VM-exits land in the `exit` module's
//!    stub; the handler emulates CPUID/MSR/CR accesses and services
//!    hypercalls (`guest::HC_*`). A stop hands the CPU back to the
//!    caller of `launch` through a saved-RSP unwind in asm.
//!
//! Intel SDM Vol. 3C is authoritative; file names/comments follow
//! MiniVisor's structure so the vendored C sources map 1:1.

#![no_std]
extern crate alloc;

pub mod asm;
pub mod cpu;
pub mod ept;
pub mod exit;
pub mod guest;
pub mod host;
pub mod vmcs;

/// CPU virtualization features as reported by CPUID/MSRs.
pub struct Caps {
    pub vmx: bool,
    pub svm: bool,
}

/// Probe the platform once at boot.
pub fn probe() -> Caps {
    let ecx = asm::cpuid(1, 0).2;
    let edx = asm::cpuid(0x8000_0001, 0).3;
    Caps { vmx: ecx & (1 << 5) != 0, svm: edx & (1 << 2) != 0 }
}

/// Caller-supplied memory for one VM. All pages must be 4 KiB-aligned
/// UEFI allocations below [`ept::COVERED_GPA`] — the identity-mapped EPT
/// only covers the low 4 GiB, and UEFI VAs equal PAs anyway.
pub struct Mmap {
    /// Per-CPU VMXON+VMCS regions ([`host::VmxCpu`], 3 pages).
    pub cpu: *mut host::VmxCpu,
    /// EPT tables ([`ept::EptTables`], 6 pages).
    pub ept: *mut ept::EptTables,
    /// MSR bitmap page (zeroed inside [`launch`]).
    pub msr_bitmap: *mut u8,
    /// Private GDT + TSS + IDT pages (2 pages) for
    /// [`host::install_tables`].
    pub tables: *mut u8,
    /// Top of a dedicated hypervisor (VM-exit) stack, 16-byte aligned.
    /// 32 KiB+ recommended.
    pub hv_stack_top: u64,
    /// Top of the guest stack. 16 KiB+ recommended.
    pub guest_stack_top: u64,
}

/// Why [`launch`]/[`resume`] failed before or during VM-entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HvError {
    /// CPUID reports no VMX.
    NoVmx,
    /// IA32_FEATURE_CONTROL can't enable VMXON outside SMX.
    FeatureControl,
    /// Couldn't fit the host GDT+TSS into the supplied page.
    Tables,
    /// vmxon instruction failed.
    Vmxon,
    /// vmclear or vmptrld failed.
    VmcsLoad,
    /// A vmwrite was rejected while filling the VMCS.
    VmcsWrite,
    /// An address the guest touches is >= 4 GiB (outside the EPT map).
    OutOfMap,
    /// vmlaunch/vmresume VMfailed; see [`exit::report`].vm_instr_err
    /// and [`exit::vmfail_flags`].
    VmEntry,
}

/// Number of 4 KiB pages to allocate for [`host::VmxCpu`].
pub const CPU_PAGES: usize = core::mem::size_of::<host::VmxCpu>() / 4096;

/// Enter the guest at `entry`. Returns `Ok(report)` when the guest hands
/// the CPU back (hypercall STOP, HLT, or a fatal VM-exit); the report
/// holds the last-exit diagnostics.
///
/// # Safety
/// `mm` must describe real, identity-mapped, exclusively-owned pages.
/// On a guest STOP the CPU returns to normal operation via vmxoff; on a
/// suspend (not currently produced) it stays rooted for [`resume`].
pub unsafe fn launch(entry: u64, mm: &Mmap) -> Result<exit::Report, HvError> {
    if !probe().vmx {
        return Err(HvError::NoVmx);
    }
    if !cpu::ensure_feature_control() {
        return Err(HvError::FeatureControl);
    }
    // Page allocations must be 4 KiB-aligned; everything the guest can
    // touch must sit inside the 4 GiB EPT map.
    if (mm.cpu as u64 | mm.ept as u64 | mm.msr_bitmap as u64 | mm.tables as u64) & 0xfff
        != 0
    {
        return Err(HvError::OutOfMap);
    }
    for &a in &[
        mm.cpu as u64,
        mm.ept as u64,
        mm.msr_bitmap as u64,
        mm.tables as u64,
        entry,
        mm.guest_stack_top,
    ] {
        if a >= ept::COVERED_GPA {
            return Err(HvError::OutOfMap);
        }
    }

    // Private GDT+TSS so guest/host TR name a real descriptor.
    let tr_sel = host::install_tables(mm.tables);
    if tr_sel == 0 {
        return Err(HvError::Tables);
    }

    let cpu_pa = mm.cpu as u64;
    let cpu = &mut *mm.cpu;
    if !cpu.init_root(cpu_pa) {
        if !cpu.is_rooted() {
            return Err(HvError::Vmxon);
        }
        return Err(HvError::VmcsLoad);
    }

    host::init_msr_bitmap(mm.msr_bitmap);
    let ept = &mut *mm.ept;
    let ept_pa = mm.ept as u64;
    ept.init_identity(ept_pa);
    let eptp = ept::eptp(ept_pa);

    let fields_ok = host::write_host_fields(
        tr_sel,
        mm.tables as u64,
        exit::uefios_vmexit_entry as *const () as usize as u64,
        mm.hv_stack_top,
    ) && {
        // The CRn read shadows hold the guest's *view* (raw values); the
        // real guest CRn fields get the FIXED-adjusted copies in
        // `write_guest_fields`.
        host::write_controls(eptp, mm.msr_bitmap as u64, asm::read_cr(0), asm::read_cr(4))
    } && guest::write_guest_fields(&guest::GuestInit {
        rip: entry,
        rsp: mm.guest_stack_top,
        tr_sel,
        gdt_base: mm.tables as u64,
    });
    if !fields_ok {
        asm::vmxoff();
        return Err(HvError::VmcsWrite);
    }

    if exit::uefios_vmx_launch() != 0 {
        return Err(HvError::VmEntry);
    }
    Ok(exit::report())
}

/// Re-enter a guest that suspended (handler returned RET_SUSPEND).
/// The VMCS must still be loaded and in the launched state.
pub unsafe fn resume() -> Result<exit::Report, HvError> {
    if exit::uefios_vmx_resume() != 0 {
        return Err(HvError::VmEntry);
    }
    Ok(exit::report())
}
