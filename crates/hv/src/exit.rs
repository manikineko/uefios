//! VM-exit handling: the host RIP the VMCS points at on every exit.
//! An outer asm stub saves GPRs + guest XMM state, calls `handle_vmexit`,
//! then either vmresumes, suspends back to the launcher, or does vmxoff.
//! Design mirrors MiniVisor's AsmHypervisorEntryPoint + HandleVmExit.

#![allow(dead_code)]

use crate::asm;
use crate::guest;
use crate::host;
use crate::vmcs;

/// Basic VM-exit reasons we dispatch on (VMCS 0x4402, low 16 bits).
pub const EXIT_EXCEPTION_NMI: u64 = 0;
pub const EXIT_TRIPLE_FAULT: u64 = 2;
pub const EXIT_CPUID: u64 = 10;
pub const EXIT_HLT: u64 = 12;
pub const EXIT_VMCALL: u64 = 18;
pub const EXIT_MOV_CR: u64 = 28;
pub const EXIT_MSR_READ: u64 = 31;
pub const EXIT_MSR_WRITE: u64 = 32;
pub const EXIT_NMI_WINDOW: u64 = 44;
pub const EXIT_EPT_VIOLATION: u64 = 48;
pub const EXIT_EPT_MISCONFIG: u64 = 49;
pub const EXIT_XSETBV: u64 = 55;

/// Handler return codes (consumed by the asm entry stub).
const RET_RESUME: i32 = 0;
const RET_SUSPEND: i32 = 1;
const RET_STOP: i32 = 2;

/// Index of each guest GPR in the stack frame the entry stub builds.
/// (Push order: rax..r15, so r15 is lowest = index 0.)
mod reg {
    pub const R15: usize = 0;
    pub const R14: usize = 1;
    pub const R13: usize = 2;
    pub const R12: usize = 3;
    pub const R11: usize = 4;
    pub const R10: usize = 5;
    pub const R9: usize = 6;
    pub const R8: usize = 7;
    pub const RDI: usize = 8;
    pub const RSI: usize = 9;
    pub const RBP: usize = 10;
    pub const RBX: usize = 11;
    pub const RDX: usize = 12;
    pub const RCX: usize = 13;
    pub const RAX: usize = 14;
}

/// Diagnostics captured on every VM-exit; also where launch-time
/// failures land. Single-vCPU for now.
#[repr(C)]
pub struct Report {
    pub exits: u64,
    pub reason: u64,
    pub qualification: u64,
    pub rip: u64,
    pub rsp: u64,
    pub rflags: u64,
    pub instr_len: u64,
    pub gpa: u64,
    /// VM-instruction error number on vmlaunch/vmresume VMfailValid.
    pub vm_instr_err: u64,
}

#[no_mangle]
static mut HV_REPORT: Report = Report {
    exits: 0,
    reason: 0,
    qualification: 0,
    rip: 0,
    rsp: 0,
    rflags: 0,
    instr_len: 0,
    gpa: 0,
    vm_instr_err: 0,
};

/// EFLAGS captured when vmlaunch/vmresume fails (CF|ZF live in here).
#[no_mangle]
static mut HV_VMFAIL_FLAGS: u64 = 0;
/// VM-instruction error number the fail stub pulls out of the VMCS.
#[no_mangle]
static mut HV_VM_INSTR_ERR: u64 = 0;
/// Host RSP to return to when a guest stops/suspends — set by the
/// launch/resume stubs before vmlaunch/vmresume.
#[no_mangle]
static mut HV_RESUME_RSP: u64 = 0;

/// Snapshot of the last exit for the caller after launch()/resume().
pub fn report() -> Report {
    unsafe {
        Report {
            exits: HV_REPORT.exits,
            reason: HV_REPORT.reason,
            qualification: HV_REPORT.qualification,
            rip: HV_REPORT.rip,
            rsp: HV_REPORT.rsp,
            rflags: HV_REPORT.rflags,
            instr_len: HV_REPORT.instr_len,
            gpa: HV_REPORT.gpa,
            vm_instr_err: HV_VM_INSTR_ERR,
        }
    }
}

/// Raw flags from the last failed vmlaunch/vmresume (bit0 CF, bit6 ZF).
pub fn vmfail_flags() -> u64 {
    unsafe { HV_VMFAIL_FLAGS }
}

// ---------------- small helpers ----------------

unsafe fn rd(field: u16) -> u64 {
    asm::vmread(field).unwrap_or(0)
}

unsafe fn advance_rip() {
    let len = rd(vmcs::VM_EXIT_INSTRUCTION_LEN);
    let rip = rd(vmcs::GUEST_RIP);
    let _ = asm::vmwrite(vmcs::GUEST_RIP, rip + len);
}

/// Deliver an event through the guest IDT on next VM-entry.
/// `typ`: 2 = NMI, 3 = hardware exception (SDM 26.5.1.3).
unsafe fn inject(vector: u32, typ: u32, err: Option<u32>) {
    let mut info = vector as u64 | ((typ as u64) << 8) | (1 << 31);
    if let Some(e) = err {
        info |= 1 << 11;
        let _ = asm::vmwrite(vmcs::VM_ENTRY_EXCEPTION_ERROR, e as u64);
    }
    let _ = asm::vmwrite(vmcs::VM_ENTRY_INTR_INFO, info);
    // Delivery is architecturally blocked while MOV-SS/STI blocking is
    // active; clear those bits so the event lands now.
    let blk = rd(vmcs::GUEST_INTERRUPTIBILITY);
    let _ = asm::vmwrite(vmcs::GUEST_INTERRUPTIBILITY, blk & !0x3);
}

unsafe fn inject_ud() {
    inject(6, 3, None);
}

/// Guest-GPR slot for the register index encoded in CR-access/qual fields.
fn reg_ptr(idx: u64) -> Option<usize> {
    Some(match idx {
        0 => reg::RAX,
        1 => reg::RCX,
        2 => reg::RDX,
        3 => reg::RBX,
        4 => return None, // guest RSP lives in the VMCS
        5 => reg::RBP,
        6 => reg::RSI,
        7 => reg::RDI,
        8 => reg::R8,
        9 => reg::R9,
        10 => reg::R10,
        11 => reg::R11,
        12 => reg::R12,
        13 => reg::R13,
        14 => reg::R14,
        15 => reg::R15,
        _ => return None,
    })
}

unsafe fn select_reg(r: &mut [u64], idx: u64) -> u64 {
    match reg_ptr(idx) {
        Some(i) => r[i],
        None => rd(vmcs::GUEST_RSP),
    }
}

unsafe fn write_reg(r: &mut [u64], idx: u64, v: u64) {
    match reg_ptr(idx) {
        Some(i) => r[i] = v,
        None => {
            let _ = asm::vmwrite(vmcs::GUEST_RSP, v);
        }
    }
}

// ---------------- per-reason handlers ----------------

unsafe fn on_cpuid(r: &mut [u64]) {
    let leaf = r[reg::RAX] as u32;
    let sub = r[reg::RCX] as u32;
    let (mut a, mut b, mut c, mut d) = asm::cpuid(leaf, sub);
    match leaf {
        1 => c &= !(1 << 5), // hide VMX — no nesting
        0x4000_0000 => {
            // Hypervisor presence signature leaf: "uefios-hv  ".
            a = 0x4000_0001;
            b = u32::from_le_bytes(*b"uefi");
            c = u32::from_le_bytes(*b"s-hv");
            d = u32::from_le_bytes(*b"os  ");
        }
        _ => {}
    }
    r[reg::RAX] = a as u64;
    r[reg::RBX] = b as u64;
    r[reg::RCX] = c as u64;
    r[reg::RDX] = d as u64;
    advance_rip();
}

unsafe fn on_vmcall(r: &mut [u64]) -> i32 {
    let nr = r[reg::RCX];
    if nr & 0xffff_ffff_0000_0000 != guest::HC_MAGIC {
        inject_ud();
        return RET_RESUME;
    }
    match nr & 0xffff_ffff {
        guest::HC_PING => {
            r[reg::RAX] = guest::HC_PING_REPLY;
            advance_rip();
            RET_RESUME
        }
        guest::HC_STOP => stop_vm(),
        _ => {
            inject_ud();
            RET_RESUME
        }
    }
}

unsafe fn on_msr_read(r: &mut [u64]) {
    let msr = r[reg::RCX] as u32;
    let v = if msr == vmcs::IA32_BIOS_SIGN_ID {
        u64::MAX // defeat guest microcode-update attempts (MiniVisor)
    } else {
        asm::rdmsr(msr)
    };
    r[reg::RAX] = v & 0xffff_ffff;
    r[reg::RDX] = v >> 32;
    advance_rip();
}

unsafe fn on_msr_write(r: &mut [u64]) {
    let msr = r[reg::RCX] as u32;
    let v = (r[reg::RAX] & 0xffff_ffff) | ((r[reg::RDX] & 0xffff_ffff) << 32);
    asm::wrmsr(msr, v);
    advance_rip();
}

unsafe fn on_cr_access(r: &mut [u64]) -> i32 {
    let qual = rd(vmcs::EXIT_QUALIFICATION);
    let cr = qual & 0xf;
    let ty = (qual >> 4) & 3;
    let gp = (qual >> 8) & 0xf;
    match ty {
        0 => {
            // mov to CR: guest keeps its view in the read shadow; the
            // real value is forced through the FIXED0/FIXED1 MSRs.
            let v = select_reg(r, gp);
            match cr {
                0 => {
                    let _ = asm::vmwrite(vmcs::CR0_READ_SHADOW, v);
                    let _ = asm::vmwrite(vmcs::GUEST_CR0, host::adjust_guest_cr0(v));
                }
                4 => {
                    let _ = asm::vmwrite(vmcs::CR4_READ_SHADOW, v);
                    let _ = asm::vmwrite(vmcs::GUEST_CR4, host::adjust_guest_cr4(v));
                }
                _ => return stop_vm(),
            }
            advance_rip();
            RET_RESUME
        }
        1 => {
            // mov from CR: hand back the shadow.
            let v = match cr {
                0 => rd(vmcs::CR0_READ_SHADOW),
                4 => rd(vmcs::CR4_READ_SHADOW),
                _ => return stop_vm(),
            };
            write_reg(r, gp, v);
            advance_rip();
            RET_RESUME
        }
        2 => {
            // CLTS: clear CR0.TS in the shadow + adjusted real value.
            let v = rd(vmcs::CR0_READ_SHADOW) & !0x8;
            let _ = asm::vmwrite(vmcs::CR0_READ_SHADOW, v);
            let _ = asm::vmwrite(vmcs::GUEST_CR0, host::adjust_guest_cr0(v));
            advance_rip();
            RET_RESUME
        }
        _ => stop_vm(), // LMSW and friends: not needed yet
    }
}

unsafe fn on_xsetbv(r: &mut [u64]) {
    // Host CR4 may lack OSXSAVE; set it so xsetbv can't #GP here.
    let cr4 = asm::read_cr(4);
    asm::write_cr4(cr4 | (1 << 18));
    let v = (r[reg::RAX] & 0xffff_ffff) | ((r[reg::RDX] & 0xffff_ffff) << 32);
    asm::xsetbv(r[reg::RCX] as u32, v);
    advance_rip();
}

unsafe fn on_exception_nmi() -> i32 {
    let info = rd(vmcs::VM_EXIT_INTERRUPTION_INFO);
    let vector = info & 0xff;
    let typ = (info >> 8) & 7;
    if typ == 2 {
        // NMI hit the guest: arm NMI-window exiting to inject it when
        // the guest can take it.
        let c = rd(vmcs::CPU_VM_EXEC_CONTROLS);
        let _ = asm::vmwrite(
            vmcs::CPU_VM_EXEC_CONTROLS,
            c | vmcs::CPU_NMI_WINDOW_EXITING,
        );
    } else {
        // Reflect the exception back into the guest.
        let err = if info & (1 << 11) != 0 {
            Some(rd(vmcs::VM_EXIT_INTERRUPTION_ERROR) as u32)
        } else {
            None
        };
        inject(vector as u32, typ as u32, err);
    }
    RET_RESUME
}

/// Shared tail for all "give the CPU back" paths: flush EPT/VPID-derived
/// translations (SDM 28.3.3.3/4 guidance) then let the stub vmxoff.
unsafe fn stop_vm() -> i32 {
    let eptp = rd(vmcs::EPT_POINTER);
    let _ = asm::invept(2, eptp); // all-context; operand ignored
    let _ = asm::invvpid(1, rd(vmcs::VIRTUAL_PROCESSOR_ID) & 0xffff, 0);
    RET_STOP
}

unsafe fn on_nmi_window() -> i32 {
    let c = rd(vmcs::CPU_VM_EXEC_CONTROLS);
    let _ = asm::vmwrite(
        vmcs::CPU_VM_EXEC_CONTROLS,
        c & !vmcs::CPU_NMI_WINDOW_EXITING,
    );
    inject(2, 2, None);
    RET_RESUME
}

/// C-level VM-exit dispatcher. Called by the asm stub with a pointer to
/// the 15 saved guest GPRs. Returns a RET_* code for the stub.
#[no_mangle]
pub extern "C" fn handle_vmexit(regs: *mut u64) -> i32 {
    let r = unsafe { core::slice::from_raw_parts_mut(regs, 15) };
    let reason = unsafe { rd(vmcs::EXIT_REASON) } & 0xffff;
    let qual = unsafe { rd(vmcs::EXIT_QUALIFICATION) };
    unsafe {
        HV_REPORT.exits += 1;
        HV_REPORT.reason = reason;
        HV_REPORT.qualification = qual;
        HV_REPORT.rip = rd(vmcs::GUEST_RIP);
        HV_REPORT.rsp = rd(vmcs::GUEST_RSP);
        HV_REPORT.rflags = rd(vmcs::GUEST_RFLAGS);
        HV_REPORT.instr_len = rd(vmcs::VM_EXIT_INSTRUCTION_LEN);
        HV_REPORT.gpa = rd(vmcs::GUEST_PHYSICAL_ADDRESS);
    }

    unsafe {
        match reason {
            EXIT_CPUID => {
                on_cpuid(r);
                RET_RESUME
            }
            EXIT_VMCALL => on_vmcall(r),
            EXIT_HLT => stop_vm(), // guest parked — take the CPU back
            EXIT_MSR_READ => {
                on_msr_read(r);
                RET_RESUME
            }
            EXIT_MSR_WRITE => {
                on_msr_write(r);
                RET_RESUME
            }
            EXIT_MOV_CR => on_cr_access(r),
            EXIT_XSETBV => {
                on_xsetbv(r);
                RET_RESUME
            }
            EXIT_EXCEPTION_NMI => on_exception_nmi(),
            EXIT_NMI_WINDOW => on_nmi_window(),
            // EPT violation/misconfig and triple fault mean the VM is
            // dead (or hit unmapped GPA) — report and stop.
            _ => stop_vm(),
        }
    }
}

// ---------------- asm stubs ----------------

// uefios_vmexit_entry: installed as HOST_RIP. On VM-exit the CPU is in
// root mode on HOST_RSP; we save guest GPRs + XMM0-15, call the handler,
// then act on its return code:
//   0 -> restore + vmresume
//   1 -> suspend: return to the launcher, VMCS stays 'launched'
//   2 -> stop: vmxoff, then return to the launcher
// uefios_vmx_launch/_resume: callable entry points that save the host's
// callee-saved regs, remember the resume RSP, and enter the guest.
core::arch::global_asm!(
    ".globl uefios_vmexit_entry",
    "uefios_vmexit_entry:",
    "pushq %rax", "pushq %rcx", "pushq %rdx", "pushq %rbx", "pushq %rbp",
    "pushq %rsi", "pushq %rdi", "pushq %r8", "pushq %r9", "pushq %r10",
    "pushq %r11", "pushq %r12", "pushq %r13", "pushq %r14", "pushq %r15",
    // XMM0-15 (guest state VMX doesn't save). 0x108 keeps 16-byte
    // alignment for movaps (15 pushes = 120 bytes, +264 -> 384).
    "subq $0x108, %rsp",
    "movaps %xmm0, 0x000(%rsp)",
    "movaps %xmm1, 0x010(%rsp)",
    "movaps %xmm2, 0x020(%rsp)",
    "movaps %xmm3, 0x030(%rsp)",
    "movaps %xmm4, 0x040(%rsp)",
    "movaps %xmm5, 0x050(%rsp)",
    "movaps %xmm6, 0x060(%rsp)",
    "movaps %xmm7, 0x070(%rsp)",
    "movaps %xmm8, 0x080(%rsp)",
    "movaps %xmm9, 0x090(%rsp)",
    "movaps %xmm10, 0x0a0(%rsp)",
    "movaps %xmm11, 0x0b0(%rsp)",
    "movaps %xmm12, 0x0c0(%rsp)",
    "movaps %xmm13, 0x0d0(%rsp)",
    "movaps %xmm14, 0x0e0(%rsp)",
    "movaps %xmm15, 0x0f0(%rsp)",
    // MS x64 ABI (what x86_64-unknown-uefi uses): arg1 in rcx,
    // 32-byte shadow space below the call site.
    "leaq 0x108(%rsp), %rcx",              // arg1 = &guest_regs[0]
    "subq $0x20, %rsp",
    "call handle_vmexit",
    "addq $0x20, %rsp",
    "movaps 0x000(%rsp), %xmm0",
    "movaps 0x010(%rsp), %xmm1",
    "movaps 0x020(%rsp), %xmm2",
    "movaps 0x030(%rsp), %xmm3",
    "movaps 0x040(%rsp), %xmm4",
    "movaps 0x050(%rsp), %xmm5",
    "movaps 0x060(%rsp), %xmm6",
    "movaps 0x070(%rsp), %xmm7",
    "movaps 0x080(%rsp), %xmm8",
    "movaps 0x090(%rsp), %xmm9",
    "movaps 0x0a0(%rsp), %xmm10",
    "movaps 0x0b0(%rsp), %xmm11",
    "movaps 0x0c0(%rsp), %xmm12",
    "movaps 0x0d0(%rsp), %xmm13",
    "movaps 0x0e0(%rsp), %xmm14",
    "movaps 0x0f0(%rsp), %xmm15",
    "addq $0x108, %rsp",
    "testl %eax, %eax",
    "jnz 2f",
    "popq %r15", "popq %r14", "popq %r13", "popq %r12", "popq %r11",
    "popq %r10", "popq %r9", "popq %r8", "popq %rdi", "popq %rsi",
    "popq %rbp", "popq %rbx", "popq %rdx", "popq %rcx", "popq %rax",
    "vmresume",
    "jmp uefios_vmx_fail",                  // vmresume VMfailed
    "2:",
    "cmpl $2, %eax",
    "jne 3f",
    "vmxoff",
    "3:",
    "xorl %eax, %eax",
    "jmp uefios_vmx_leave",
    // ---- enter/leave helpers -------------------------------------
    // MS x64 callee-saved regs: rbp, rbx, rdi, rsi, r12-r15 — a guest
    // may clobber any GPR, so save all of them before entering.
    ".globl uefios_vmx_launch",
    "uefios_vmx_launch:",
    "pushq %rbp", "pushq %rbx", "pushq %rdi", "pushq %rsi",
    "pushq %r12", "pushq %r13", "pushq %r14", "pushq %r15",
    "movq %rsp, HV_RESUME_RSP(%rip)",
    "vmlaunch",
    "jmp uefios_vmx_fail",
    ".globl uefios_vmx_resume",
    "uefios_vmx_resume:",
    "pushq %rbp", "pushq %rbx", "pushq %rdi", "pushq %rsi",
    "pushq %r12", "pushq %r13", "pushq %r14", "pushq %r15",
    "movq %rsp, HV_RESUME_RSP(%rip)",
    "vmresume",
    "jmp uefios_vmx_fail",
    // vmlaunch/vmresume VMfailed: capture flags (+error nr if ZF), then
    // return to the Rust caller with rax = -1.
    "uefios_vmx_fail:",
    "pushfq",
    "popq %rax",
    "movq %rax, HV_VMFAIL_FLAGS(%rip)",
    "testl $0x40, %eax",
    "jz 9f",
    "movq $0x4400, %rcx",
    "vmread %rcx, %rax",
    "movq %rax, HV_VM_INSTR_ERR(%rip)",
    "9:",
    "movq $-1, %rax",
    // Shared tail: switch to the launch-time stack and return to the
    // Rust caller of uefios_vmx_launch/_resume (rax = status).
    "uefios_vmx_leave:",
    "movq HV_RESUME_RSP(%rip), %rsp",
    "popq %r15", "popq %r14", "popq %r13", "popq %r12",
    "popq %rsi", "popq %rdi", "popq %rbx", "popq %rbp",
    "ret",
    options(att_syntax),
);

extern "C" {
    /// Symbol for the VMExit entrypoint; pass `as usize as u64` as HOST_RIP.
    pub fn uefios_vmexit_entry();
    /// vmlaunch: returns 0 when the guest gave the CPU back, -1 on VMfail.
    pub fn uefios_vmx_launch() -> i64;
    /// vmresume: same contract for re-entering a suspended guest.
    pub fn uefios_vmx_resume() -> i64;
}
