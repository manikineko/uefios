#!/usr/bin/env bash
# uefios one-stop script for Linux (and macOS, best-effort).
#
#   ./uefios.sh setup     # install Rust target, OVMF firmware; print QEMU hints
#   ./uefios.sh build     # build the OS -> esp/EFI/BOOT/BOOTX64.EFI
#   ./uefios.sh run       # boot it in QEMU (builds first if needed)
#   ./uefios.sh test      # host-side unit tests
#   ./uefios.sh clean     # cargo clean + staged EFI binaries
#
# Env overrides: QEMU=path/to/qemu-system-x86_64, RELEASE=1 (or pass `release`)

set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OVMF_DIR="$ROOT/firmware/ovmf"
OVMF_BASE="https://raw.githubusercontent.com/retrage/edk2-nightly/master/bin"

TASK="${1:-help}"
RELEASE="${RELEASE:-}"
[ "${2:-}" = "release" ] && RELEASE=1

say() { printf '==> %s\n' "$*"; }
die() { printf 'error: %s\n' "$*" >&2; exit 1; }

find_qemu() {
    if [ -n "${QEMU:-}" ] && [ -x "$QEMU" ]; then echo "$QEMU"; return; fi
    command -v qemu-system-x86_64 2>/dev/null && return
    for p in /usr/bin/qemu-system-x86_64 /usr/local/bin/qemu-system-x86_64 \
             /opt/homebrew/bin/qemu-system-x86_64; do
        [ -x "$p" ] && { echo "$p"; return; }
    done
    return 1
}

get_ovmf() {
    mkdir -p "$OVMF_DIR"
    for pair in "OVMF_CODE.fd:RELEASEX64_OVMF_CODE.fd" \
                "OVMF_VARS.fd:RELEASEX64_OVMF_VARS.fd"; do
        name="${pair%%:*}"; url="${pair##*:}"
        [ -f "$OVMF_DIR/$name" ] || { echo "  downloading $name"; \
            curl -fsSL -o "$OVMF_DIR/$name" "$OVMF_BASE/$url"; }
    done
}

qemu_hint() {
    if   command -v apt-get >/dev/null;  then echo "  sudo apt install qemu-system-x86 ovmf";
    elif command -v dnf >/dev/null;      then echo "  sudo dnf install qemu-system-x86 edk2-ovmf";
    elif command -v pacman >/dev/null;   then echo "  sudo pacman -S qemu-full edk2-ovmf";
    elif command -v brew >/dev/null;     then echo "  brew install qemu";
    else echo "  install qemu-system-x86_64 with your package manager"; fi
}

task_setup() {
    say "Rust toolchain"
    if ! command -v cargo >/dev/null; then
        echo "  cargo not found -- installing rustup (stable, minimal profile)"
        curl -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain stable --profile minimal
        # shellcheck disable=SC1091
        . "$HOME/.cargo/env"
    fi
    cargo --version
    say "x86_64-unknown-uefi target"
    rustup target add x86_64-unknown-uefi

    say "OVMF firmware -> firmware/ovmf/"
    get_ovmf

    say "QEMU"
    if q=$(find_qemu); then echo "  found: $q"; else qemu_hint; fi
    echo "setup done. next: ./uefios.sh build && ./uefios.sh run"
}

task_build() {
    args=(build --manifest-path "$ROOT/crates/Cargo.toml" -p os)
    profile=debug
    if [ -n "$RELEASE" ]; then args+=(--release); profile=release; fi
    cargo "${args[@]}"

    efi="$ROOT/crates/target/x86_64-unknown-uefi/$profile/os.efi"
    [ -f "$efi" ] || die "expected build output missing: $efi"
    mkdir -p "$ROOT/esp/EFI/BOOT"
    cp -f "$efi" "$ROOT/esp/EFI/BOOT/BOOTX64.EFI"
    say "staged $ROOT/esp/EFI/BOOT/BOOTX64.EFI"
}

task_run() {
    [ -f "$ROOT/esp/EFI/BOOT/BOOTX64.EFI" ] || { say "BOOTX64.EFI missing -- building first"; task_build; }
    qemu="$(find_qemu)" || die "qemu-system-x86_64 not found -- run ./uefios.sh setup or set QEMU=/path"

    code="$OVMF_DIR/OVMF_CODE.fd"
    vars="$OVMF_DIR/OVMF_VARS.local.fd"
    [ -f "$code" ] || get_ovmf
    [ -f "$vars" ] || cp "$OVMF_DIR/OVMF_VARS.fd" "$vars"

    # The bundled OVMF is a Secure-Boot build and requires SMM when variables
    # live in pflash: smm=on + secure pflash. KVM supports SMM; macOS HVF and
    # Windows WHPX do not, so those fall back to TCG.
    case "$(uname -s)" in
        Linux) accel="kvm:tcg" ;;
        *)     accel="tcg" ;;
    esac

    "$qemu" \
        -machine "q35,smm=on,accel=$accel" \
        -cpu max \
        -m 512M \
        -global driver=cfi.pflash01,property=secure,value=on \
        -drive "if=pflash,format=raw,readonly=on,file=$code" \
        -drive "if=pflash,format=raw,file=$vars" \
        -drive "format=raw,file=fat:rw:$ROOT/esp" \
        -device qemu-xhci -usb -device usb-tablet \
        -net none \
        -serial stdio
}

task_test() {
    host="$(rustc -vV | awk '/^host:/ {print $2}')"
    cargo test --manifest-path "$ROOT/crates/Cargo.toml" --target "$host" -p shell
}

task_clean() {
    cargo clean --manifest-path "$ROOT/crates/Cargo.toml"
    rm -f "$ROOT/esp/EFI/BOOT/BOOTX64.EFI"
    say "cleaned"
}

case "$TASK" in
    setup) task_setup ;;
    build) task_build ;;
    run)   task_run ;;
    test)  task_test ;;
    clean) task_clean ;;
    *)     echo "usage: ./uefios.sh {setup|build|run|test|clean} [release]" ;;
esac
