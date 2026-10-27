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
