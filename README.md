# uefios

A kernelless bare-metal operating system that runs **entirely as a UEFI
application**. The firmware's boot services are the kernel: memory allocation,
filesystems, graphics, input, and networking are all consumed through UEFI
protocols instead of a traditional kernel.

On top of that foundation:

- **GUI** — a compositing window manager over the GOP framebuffer (`wm`)
- **Unix-ish shell** — builtin commands now; FreeBSD-derived userland later (`shell`)
- **Hypervisor "containers"** — a VT-x/AMD-V type-1 hypervisor that boots guest
  OSes whose displays appear as windows in the GUI (`hv`)
- **Legacy BIOS path** — a DUET-style UEFI environment loaded from a BIOS boot
  sector, so the same OS image runs on pre-UEFI hardware (`firmware/`)

## Layout

```
crates/          Rust workspace (x86_64-unknown-uefi)
  os/            the OS itself — BOOTX64.EFI entry point
  wm/            window manager / compositor (GOP framebuffer)
  shell/         shell parser + builtins (no_std, host-testable)
  hv/            type-1 hypervisor (Intel VT-x first, AMD-V planned)
esp/             staged EFI System Partition (BOOTX64.EFI lands here)
firmware/        EDK II side: UEFI shell pieces, DUET legacy-BIOS shim
tools/           build.ps1 / run-qemu.ps1
docs/            ARCHITECTURE.md, ROADMAP.md, BUILDING.md
```

## Quickstart

```powershell
.\uefios.ps1 setup    # once: rust UEFI target, OVMF firmware, QEMU
.\uefios.ps1 build    # -> esp\EFI\BOOT\BOOTX64.EFI
.\uefios.ps1 run      # boot in QEMU/OVMF
```

Linux/macOS: same subcommands via `./uefios.sh`.
Details: [docs/BUILDING.md](docs/BUILDING.md).
Design details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
Milestones: [docs/ROADMAP.md](docs/ROADMAP.md).
