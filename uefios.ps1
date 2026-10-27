#Requires -Version 5.1
<#
.SYNOPSIS
  uefios one-stop script for Windows.

.EXAMPLE
  .\uefios.ps1 setup     # install Rust target, OVMF firmware, QEMU (via winget)
  .\uefios.ps1 build     # build the OS -> esp\EFI\BOOT\BOOTX64.EFI
  .\uefios.ps1 run       # boot it in QEMU (builds first if needed)
  .\uefios.ps1 test      # host-side unit tests
  .\uefios.ps1 clean     # cargo clean + unstaged EFI binaries
#>
param(
    [Parameter(Position = 0)]
    [ValidateSet("help", "setup", "build", "run", "test", "clean")]
    [string]$Task = "help",
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
$OvmfDir = Join-Path $Root "firmware\ovmf"
$OvmfBase = "https://raw.githubusercontent.com/retrage/edk2-nightly/master/bin"

function Find-Qemu {
    if ($env:QEMU -and (Test-Path $env:QEMU)) { return $env:QEMU }
    $cmd = Get-Command qemu-system-x86_64 -ErrorAction SilentlyContinue
    if ($cmd) { return $cmd.Source }
    $pf = Join-Path $env:ProgramFiles "qemu\qemu-system-x86_64.exe"
    if (Test-Path $pf) { return $pf }
    return $null
}

function Get-Ovmf {
    New-Item -ItemType Directory -Force $OvmfDir | Out-Null
    foreach ($f in @(
        @{ Name = "OVMF_CODE.fd";  Url = "$OvmfBase/RELEASEX64_OVMF_CODE.fd" },
        @{ Name = "OVMF_VARS.fd";  Url = "$OvmfBase/RELEASEX64_OVMF_VARS.fd" }
    )) {
        $dst = Join-Path $OvmfDir $f.Name
        if (-not (Test-Path $dst)) {
            Write-Host "  downloading $($f.Name)"
            Invoke-WebRequest -UseBasicParsing -Uri $f.Url -OutFile $dst
        }
    }
}

function Task-Setup {
    Write-Host "==> Rust toolchain"
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        Write-Host "  cargo not found -- installing rustup (stable, minimal profile)"
        $tmp = Join-Path $env:TEMP "rustup-init.exe"
        Invoke-WebRequest -UseBasicParsing -Uri "https://win.rustup.rs/x86_64" -OutFile $tmp
        & $tmp -y --default-toolchain stable --profile minimal
        if ($LASTEXITCODE -ne 0) { throw "rustup-init failed" }
        $env:Path = "$env:USERPROFILE\.cargo\bin;$env:Path"
    }

    # Host build scripts (proc-macros) need a linker. The MSVC toolchain
    # requires Visual Studio Build Tools; the GNU toolchain ships its own
    # linker, so we pin it as a directory-local override (not a global change).
    Write-Host "==> windows-gnu host toolchain (bundled linker, no VS required)"
    rustup toolchain install stable-x86_64-pc-windows-gnu --profile minimal
    Push-Location "$Root\crates"
    try { rustup override set stable-x86_64-pc-windows-gnu } finally { Pop-Location }
    cargo --version

    Write-Host "==> x86_64-unknown-uefi target"
    rustup target add x86_64-unknown-uefi

    Write-Host "==> OVMF firmware -> firmware\ovmf\"
    Get-Ovmf

    Write-Host "==> QEMU"
    if (Find-Qemu) {
        Write-Host "  found: $(Find-Qemu)"
    } else {
        Write-Host "  not found -- installing via winget (may prompt for admin)"
        winget install -e --id SoftwareFreedomConservancy.QEMU `
            --accept-source-agreements --accept-package-agreements
    }
    Write-Host "setup done. next: .\uefios.ps1 build ; .\uefios.ps1 run"
}

function Task-Build {
    $cargoArgs = @("build", "--manifest-path", "$Root\crates\Cargo.toml", "-p", "os")
    $profile = "debug"
    if ($Release) { $cargoArgs += "--release"; $profile = "release" }
    & cargo @cargoArgs
    if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

    $efi = "$Root\crates\target\x86_64-unknown-uefi\$profile\os.efi"
    if (-not (Test-Path $efi)) { throw "expected build output missing: $efi" }
    $dest = "$Root\esp\EFI\BOOT"
    New-Item -ItemType Directory -Force $dest | Out-Null
    Copy-Item $efi "$dest\BOOTX64.EFI" -Force
    Write-Host "==> staged $dest\BOOTX64.EFI"
}

function Task-Run {
    if (-not (Test-Path "$Root\esp\EFI\BOOT\BOOTX64.EFI")) {
        Write-Host "==> BOOTX64.EFI missing -- building first"
        Task-Build
    }
    $qemu = Find-Qemu
    if (-not $qemu) { throw "qemu-system-x86_64 not found -- run '.\uefios.ps1 setup' or set `$env:QEMU" }

    $code = Join-Path $OvmfDir "OVMF_CODE.fd"
    $vars = Join-Path $OvmfDir "OVMF_VARS.local.fd"
    if (-not (Test-Path $code)) { Get-Ovmf }
    if (-not (Test-Path $vars)) {
        Copy-Item (Join-Path $OvmfDir "OVMF_VARS.fd") $vars
    }

    # The bundled OVMF is a Secure-Boot build: it requires SMM when variables
    # live in pflash, which means smm=on + secure pflash. Windows' WHPX
    # hypervisor cannot emulate SMM, so TCG is the only option on Windows.
    & $qemu `
        -machine "q35,smm=on,accel=tcg" `
        -cpu max `
        -m 512M `
        -global driver=cfi.pflash01,property=secure,value=on `
        -drive "if=pflash,format=raw,readonly=on,file=$code" `
        -drive "if=pflash,format=raw,file=$vars" `
        -drive "format=raw,file=fat:rw:$Root\esp" `
        -device qemu-xhci -usb -device usb-tablet `
        -net none `
        -serial stdio
}

function Task-Test {
    $host = (rustc -vV | Select-String '^host:').ToString().Split()[1]
    & cargo test --manifest-path "$Root\crates\Cargo.toml" `
        --target $host -p shell
    if ($LASTEXITCODE -ne 0) { throw "tests failed" }
}

function Task-Clean {
    & cargo clean --manifest-path "$Root\crates\Cargo.toml"
    Remove-Item "$Root\esp\EFI\BOOT\BOOTX64.EFI" -Force -ErrorAction SilentlyContinue
    Write-Host "==> cleaned"
}

switch ($Task) {
    "setup" { Task-Setup }
    "build" { Task-Build }
    "run"   { Task-Run }
    "test"  { Task-Test }
    "clean" { Task-Clean }
    default {
        Write-Host "usage: .\uefios.ps1 {setup|build|run|test|clean} [-Release]"
    }
}
