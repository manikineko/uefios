# Porting FreeBSD `sh` (Almquist shell) to uefios

Sources vendored at `userland/freebsd-src/bin/sh/` (sparse clone, BSD-2).

## The hard problem: no fork()

sh's core model — pipelines, `&`, command substitution — assumes
fork()/exec()/wait()/pipe(). UEFI Boot Services has no processes.
Our process model, per docs/ARCHITECTURE.md:

- "Processes" = child EFI images launched via `LoadImage`/`StartImage`
  (boot services, no ExitBootServices needed) — synchronous wait only.
- A custom `UEFIOS_SHELL_PROTOCOL` handle pairs a child app with a shell
  window: read/write pipe over console handles instead of fd 0/1/2.
- Pipelines = sequential stages; later stages read the previous stage's
  buffered output (no streaming pipes in v1). Job control is stubbed.

## The libc shim (`userland/libc-uefi/`)

sh needs a POSIX C runtime. Two routes:

- **EDK II StdLib (StdLibPrivateInternalFiles)** — drop-in libc for
  UEFI C apps. Biggest coverage; heavy weight. Requires EDK2 build tree.
- **Custom thin shim** — implement only what sh touches:
  read/write/open/close/lstat/stat/opendir (→ Simple File System),
  malloc/free (→ AllocatePool), tcsetattr/setpgid/etc (→ no-op stubs),
  kill/raise (→ signals→events), exit (→ return from main), environ.

## C toolchain for UEFI

clang -target x86_64-unknown-windows -mno-red-zone -fno-stack-protector
link: lld-link -subsystem:efi_application -entry:efi_main → sh.efi
Proven pattern used by uefi-app projects; no EDK2 required.

## Files in sh worth porting first

input.c output.c error.c parser.c alias.c expand.c eval.c exec.c
(jobs.c mail.c histedit.c cd.c are stubbable/optional in v1)
builtins.def drives the builtin table — almost all become real.
