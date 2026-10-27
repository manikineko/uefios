# Building & running

One script per platform, same subcommands:

| Windows                    | Linux / macOS              | What it does                                     |
| -------------------------- | -------------------------- | ------------------------------------------------ |
| `.\uefios.ps1 setup`       | `./uefios.sh setup`        | Rust UEFI target, OVMF firmware download, QEMU   |
| `.\uefios.ps1 build`       | `./uefios.sh build`        | cargo build → `esp/EFI/BOOT/BOOTX64.EFI`          |
| `.\uefios.ps1 run`         | `./uefios.sh run`          | boot in QEMU/OVMF (builds first if needed)       |
| `.\uefios.ps1 test`        | `./uefios.sh test`         | host-side unit tests                             |
| `.\uefios.ps1 clean`       | `./uefios.sh clean`        | cargo clean + staged binaries                    |

Add `-Release` / `release` argument for release builds.

## What setup does

- **Rust** — installs rustup if missing, adds `x86_64-unknown-uefi`.
  On Windows it also installs `stable-x86_64-pc-windows-gnu` and sets it as a
  *directory-local* `rustup override` inside `crates/`. Reason: host-side
  build scripts (proc-macros) need a linker; MSVC's `link.exe` only exists if
  Visual Studio Build Tools are installed, while the GNU toolchain ships its
  own. To opt out: `cd crates; rustup override unset`.
- **OVMF** — downloads `OVMF_CODE.fd` / `OVMF_VARS.fd` (retrage/edk2-nightly
  release builds) into `firmware/ovmf/`. No distro OVMF package needed; these
  work on every OS.
- **QEMU** — Windows: `winget install SoftwareFreedomConservancy.QEMU`.
  Linux: prints the right package-manager line (`qemu-system-x86`/`qemu-full`).
  Override with `QEMU=/path/to/qemu-system-x86_64` / `$env:QEMU`.

## QEMU invocation details (why the flags)

```
-machine q35,smm=on,accel=kvm:tcg   # accel=t tcg on Windows/macOS
-global driver=cfi.pflash01,property=secure,value=on
-drive if=pflash,format=raw,readonly=on,file=OVMF_CODE.fd
-drive if=pflash,format=raw,file=OVMF_VARS.local.fd
-drive format=raw,file=fat:rw:esp   # virtual FAT: no disk image needed
```

The bundled OVMF is a **Secure Boot-capable build**: with variables stored in
pflash it requires SMM (`smm=on` + `secure` pflash property) or it asserts in
`QemuFlash.c` and hangs before display init. KVM supports SMM; Windows' WHPX
and macOS' HVF do **not**, so those hosts run TCG (fine for this workload).

## Real hardware

Copy `esp/` to the root of a FAT32 USB stick (`EFI\BOOT\BOOTX64.EFI`), boot it
from the UEFI boot menu. Secure Boot must be disabled — binaries are unsigned.
