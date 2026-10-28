# Project notes for agents

## Build / verify

- `.\uefios.ps1 build` (Windows) / `./uefios.sh build` (Linux) →
  `esp/EFI/BOOT/BOOTX64.EFI`
- `.\uefios.ps1 run` / `./uefios.sh run` boots QEMU+OVMF headful
- Host unit tests: `uefios test` (uses `rustc -vV` host triple — the
  workspace pins the UEFI target, so tests must override `--target`)
- Windows dev loop: workspace uses `stable-x86_64-pc-windows-gnu` via a
  rustup directory override (crates/), because MSVC link.exe needs VS Build
  Tools and GNU ships its own linker
- Pinned dep: `uefi = "0.35"` — check docs.rs/uefi/0.35.0 before using new
  APIs; the API changed substantially across minor versions
- QEMU gotchas baked into the scripts: bundled OVMF needs `smm=on` +
  `cfi.pflash01 secure=on`; KVM-only accel (no WHPX/HVF — they lack SMM).
  Headless debugging: `-debugcon file:... -global isa-debugcon.iobase=0x402`
  with the DEBUGX64 OVMF build; `screendump` via `-monitor tcp:...` works
  (one monitor client at a time)

## Conventions

- All Rust crates are `#![no_std]`; `alloc` via UEFI pool allocator
  (`global_allocator` feature)
- Never call `ExitBootServices` — UEFI boot services ARE the kernel; the OS
  entry point also never returns (resident loop)
- `shell` and `hv` stay free of `uefi` deps so they can be host-tested
- Design decisions and milestone order live in docs/ — update ROADMAP.md when
  a phase completes

## Post-scaffold additions (session 2)

- wm: Compositor (dirty-rect, direct-fb writes + blt fallback), Window
  chrome, font8x8 (unicode feature required), cursor bitmap,
  AbsolutePointer FFI (field MUST be inline UnsafeCell<Raw>, not *mut).
- shell: stateful REPL, revision-bump repaint model.
- os: event loop; input via GetProtocol (never exclusive — deadlocks
  OVMF); window `id` field for routing (focus() reorders windows[]).
- qemu-mon.py: monitor client (shot/type/move/click). NOTE: sendkey/
  mouse_move do NOT reach guest under headless -vnc without client —
  use QMP input-send-event or a real VNC client to verify input.
- userland/freebsd-src: sparse clone of freebsd bin/sh; PORTING.md
  documents the no-fork port design (LoadImage/StartImage process model).

## Session 3 additions (input + loader verified)

- Input requires `-device virtio-keyboard-pci -device virtio-tablet-pci`:
  this OVMF build has NO PS/2 mouse driver, and its PS/2 kbd path doesn't
  feed ConIn reliably. VirtioInputDxe produces SimpleTextInput + an
  AbsolutePointer — verified working via QMP `input-send-event`.
- qemu-qmp.py: input-send-event client (type/key/abs/rel/btn) — works
  headless; HMP sendkey does NOT inject reliably.
- Never use `boot::stall` in the event loop: OVMF's Stall spins on the
  ACPI PM timer which doesn't advance under TCG -> freezes. Use a spin
  loop for frame pacing.
- usb-tablet's AbsolutePointer::get_state BLOCKS in a URB poll — don't
  use it; virtio-tablet's is event-driven and safe.
- File manager + .efi launch verified live: keys navigate, click focuses
  (taskbar shows focused title), Enter on HELLO.EFI ran the guest via
  LoadImage/StartImage and returned to the desktop.
