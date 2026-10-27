# Roadmap

Ordered by dependency. Each phase ends with something bootable and demoable.

## Phase 0 — Scaffold (done)

Rust workspace, crates, tooling, docs. `os` prints diagnostics and paints a
demo desktop via GOP.

## Phase 1 — Interactive text shell

- REPL in `os`: read keys via Simple Text Input Ex, line editor, history
- Builtins in `shell`: `help`, `ver`, `uname`, `clear`, `efiinfo`,
  `ls`/`cd` over the Simple File System protocol, `exit`/`reboot`
- Verify: boot in QEMU, run commands, browse `fs0:`

## Phase 2 — Framebuffer console + task model

- Bitmap/pixel font renderer in `wm`; text console drawn into GOP framebuffer
- Cooperative task abstraction (spawn/yield on `WaitForEvent`)
- Verify: shell runs fullscreen in graphics mode, two "apps" timeshare

## Phase 3 — GUI

- `Surface` + window tree + dirty-rect compositor in `wm`
- Mouse via Absolute Pointer protocol; drag/focus
- Window chrome, taskbar (the demo paint becomes real)
- Verify: shell window + a second app window, movable

## Phase 4 — Unix environment

- Pick track: FreeBSD userland port vs. EDK II StdLib/ShellPkg reuse
- `sh` (Almquist) running; basic utilities; filesystem layout on ESP
- Verify: `sh` script executes, pipelines work

## Phase 5 — Hypervisor

- `hv`: feature-control MSR check, VMXON, VMCS, EPT, `vmlaunch`
- Tiny guest first (16-bit or flat protected code), then a real bootloader
- Guest framebuffer trapped/emulated → `wm` window
- Verify: two guest windows running side by side in the GUI

## Phase 6 — Legacy BIOS path

- `firmware/`: BIOS boot sector -> loader -> DUET-style UEFI environment
- BIOS-backed protocol implementations (int13/int10/int16)
- Verify: same `BOOTX64.EFI` boots under SeaBIOS on a legacy-only VM

## Phase 7 — Depth

- SMP via MP Services, >1 vCPU guests
- Networking: SNP stack or a ported stack
- Guest tooling: images, config files, snapshotting
- Hardening: memory hygiene around firmware calls, watchdog recovery

## Session 3 status

DONE: File manager window browses fs0: via SimpleFS (dirs first, sel
highlight, Enter opens dir / queues .efi launch, Backspace up a dir).
DONE: App-loader "container v0" — launch .efi via LoadImage/StartImage;
guest shares ConIn/ConOut/GOP and host resumes on exit (hello crate
staged at esp/TOOLS/HELLO.EFI proves the path).
TODO next: wire keyboard to FM focus (sendkey needs real VNC/QMP),
streaming pipes between child apps, real VT-x in hv/, libc-uefi shim
for bin/sh.
