# firmware/ — the EDK II side of the hybrid toolchain

Everything the Rust core can reasonably reuse instead of rewrite lives here.
This directory becomes an EDK II workspace (or a submodule set) at the phase
where each piece is needed.

## What goes here

| Component   | Purpose                                              | Needed at |
| ----------- | ---------------------------------------------------- | --------- |
| ShellPkg    | Reference UEFI shell — command implementations       | Phase 4   |
| StdLib      | POSIX libc over UEFI (base for ported userland)      | Phase 4   |
| DUET shim   | UEFI environment on top of legacy BIOS interrupts    | Phase 6   |

## EDK II workspace setup (Windows, when we get there)

Prereqs: Visual Studio Build Tools (or set `TOOL_CHAIN_TAG=CLANGPDB` with
Clang), NASM, Python 3, IASL (for ACPI-related packages). Then:

```powershell
git clone --recurse-submodules https://github.com/tianocore/edk2.git
cd edk2
edksetup.bat
build -a X64 -p ShellPkg\ShellPkg.dsc -t VS2022
```

Produces `Shell.efi` — usable as a command reference and as a fallback shell
on the ESP.

## DUET reality check (phase 6)

DUET (Developer's UEFI Emulation) boots a UEFI DXE environment from a legacy
BIOS. Upstream `DuetPkg` was dropped from edk2 mainline years ago; maintained
descendants live in the **Clover** bootloader's `DuetPkg` fork and various
community ports. Options, cheapest first:

1. Fork Clover's DuetPkg, strip it to "provide UEFI services over BIOS"
2. Use Clover itself to chainload our DUET image during development
3. Long-term: own minimal shim — BIOS boot sector -> loader -> DXE core +
   int13/int10/int16-backed drivers -> `BOOTX64.EFI`

Either way the contract is identical: build a UEFI environment good enough
that `crates/os` cannot tell it isn't running on native firmware.
