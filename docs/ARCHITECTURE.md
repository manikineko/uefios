# Architecture

## The core idea: "kernelless"

uefios never calls `ExitBootServices`. A conventional OS exits boot services,
takes over memory and interrupts, and reimplements drivers itself. uefios
instead treats the **UEFI boot services as its kernel**:

| Kernel function        | UEFI equivalent                                        |
| ---------------------- | ------------------------------------------------------ |
| Physical page alloc    | `AllocatePages` / `FreePages`                           |
| Heap                   | `AllocatePool` / `FreePool` (Rust global allocator)     |
| Filesystem             | Simple File System protocol (+ firmware FAT driver)     |
| Graphics               | Graphics Output Protocol (linear framebuffer)           |
| Keyboard/pointer       | Simple Text Input Ex, Simple/Absolute Pointer protocols |
| Block/network drivers  | Loaded by firmware from device ROMs — we inherit them   |
| Timers                 | `CreateEvent`/`SetTimer`, `Stall`                       |
| Power/reset            | Runtime Services `ResetSystem`                          |

Consequences, stated honestly:

- **Pro**: hardware support for free. Any machine that boots UEFI already
  shipped the drivers. A "kernelless" OS is real because the kernel work was
  done by the firmware vendor.
- **Con**: we inherit firmware quirks and single-threaded boot-services
  semantics. Preemptive multitasking does not exist in boot services — UEFI is
  a cooperative, event-driven (`WaitForEvent`) environment. Scheduling must be
  built on top (cooperative tasks first; true preemption requires taking over
  the APIC/timer, which is a deliberate later escalation).
- **Con**: memory-map ownership stays with the firmware. Fine — `AllocatePool`
  is all a GUI/shell needs.

## Crate map (Rust side)

- **`os`** — the EFI application. Owns the main loop, event dispatch, and wires
  the other crates together. Links into `BOOTX64.EFI`.
- **`wm`** — window manager + compositor. GOP provides the framebuffer; this
  crate provides surfaces, a z-ordered window tree, dirty-rect compositing, and
  input routing. Guests (from `hv`) present as windows.
- **`shell`** — parser and builtins, target-agnostic `no_std` so it unit-tests
  on the host. Grows into the POSIX command environment.
- **`hv`** — type-1 hypervisor. See below.

## The "containerization" piece is a hypervisor

Running multiple OSes simultaneously on bare metal requires hardware
virtualization — there is no container shortcut without a kernel. The design:

```
            uefios (VMX root mode)
   +-------------------------------------------+
   |  wm compositor   |  shell  |  hv core     |
   +------------------|---------|--------------+
        window 1      |  window 2    window 3
        [guest A]     |  [guest B]   [shell]
   +-------------------------------------------+
   |  VMCS / EPT / vmexit handlers (non-root)   |
   +-------------------------------------------+
```

- Host runs VMX root; guests run non-root with EPT second-level translation.
- Guest display: initially VGA text/framebuffer emulation via EPT-trapped MMIO,
  later a paravirt display device (guest issues VM calls; host shares a
  surface that `wm` composites).
- Precedent: SimpleVisor, hv-as-UEFI-app, BitVisor — a UEFI-hosted type-1
  hypervisor is proven territory.
- AMD support follows the same structure via SVM/VMRUN.

## Unix layer

Two tracks, evaluated in parallel at the relevant milestone:

1. **Port FreeBSD userland.** `sh` is the Almquist shell — small and portable.
   Coreutils pieces (`ls`, `cat`, ...) on top of a libc shim backed by UEFI
   protocols. FreeBSD kernel code is *not* portable here (that's the point of
   kernelless); userland is.
2. **EDK II StdLib + ShellPkg.** TianoCore already ships a POSIX-flavored libc
   and a full UEFI shell in C. Reusing it via the `firmware/` tree trades some
   purity for a lot of working commands immediately.

## Legacy BIOS path (`firmware/`)

The "non-UEFI version" is not a second OS — it is a **UEFI implementation
loaded by a BIOS bootloader** (the DUET approach, also used by Clover):

```
BIOS boot sector -> real-mode loader -> protected/long mode
                 -> minimal DXE core + BIOS-backed drivers
                   (int13 disk, int10 video, int16 kbd wrapped as protocols)
                 -> BOOTX64.EFI (the same uefios image, unchanged)
```

Every UEFI protocol uefios consumes gets a BIOS-interrupt-backed
implementation. Guest virtualization still works — VT-x is CPU hardware,
independent of boot firmware — though real-mode-booted machines old enough to
lack VT-x simply get `hv` disabled.

## Open design decisions

- **Cooperative vs. preemptive tasks** — boot services are single-threaded.
  True preemption means owning the IDT/LAPIC timer while keeping boot services
  callable (doable; firmware code is reentrant-when-told). Deferred until the
  GUI needs it.
- **Guest device model** — emulated-VGA vs paravirt-display-first. Paravirt is
  far less code; emulated VGA boots unmodified OSes.
- **SMP** — UEFI MP Services protocol can wake APs without exiting boot
  services. Needed before guests get >1 vCPU.
