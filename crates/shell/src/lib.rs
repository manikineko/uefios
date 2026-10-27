#![no_std]

//! The uefios shell.
//!
//! This crate is deliberately target-agnostic (`no_std` + `alloc` only) so the
//! parser and builtins can be unit-tested on the host. The `os` crate owns the
//! REPL loop: read keys via the UEFI console protocols, feed lines here, and
//! dispatch.
//!
//! Long-term direction: a POSIX-ish command environment with FreeBSD-derived
//! userland (`sh`/Almquist shell, coreutils) over a libc shim — see
//! docs/ARCHITECTURE.md. `Builtin` is the seed of that surface.

extern crate alloc;

use alloc::string::String;
use alloc::vec::Vec;

/// A whitespace-tokenized command line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parsed {
    pub argv: Vec<String>,
}

impl Parsed {
    /// The command name (argv\[0]), if any.
    pub fn name(&self) -> Option<&str> {
        self.argv.first().map(String::as_str)
    }
}

/// Tokenize `line` on whitespace. Returns `None` for empty input.
///
/// Phase 1 keeps this deliberately dumb — no quoting, globbing, pipes, or
/// escapes. Those arrive with the real `sh` port.
pub fn parse(line: &str) -> Option<Parsed> {
    let argv: Vec<String> = line.split_whitespace().map(String::from).collect();
    if argv.is_empty() {
        None
    } else {
        Some(Parsed { argv })
    }
}

/// Commands implemented natively by the OS rather than by ported userland.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Help,
    Version,
    Uname,
    Clear,
    EfiInfo,
    Exit,
}

/// Resolve a command name to a builtin, if one exists.
pub fn builtin(name: &str) -> Option<Builtin> {
    Some(match name {
        "help" | "?" => Builtin::Help,
        "ver" | "version" => Builtin::Version,
        "uname" => Builtin::Uname,
        "clear" | "cls" => Builtin::Clear,
        "efiinfo" => Builtin::EfiInfo,
        "exit" | "quit" => Builtin::Exit,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_empty_input() {
        assert!(parse("").is_none());
        assert!(parse("   \t ").is_none());
    }

    #[test]
    fn parse_splits_on_whitespace() {
        let p = parse("ls -l fs0:").unwrap();
        assert_eq!(p.name(), Some("ls"));
        assert_eq!(p.argv.len(), 3);
    }

    #[test]
    fn builtins_resolve_and_reject_unknown() {
        assert_eq!(builtin("uname"), Some(Builtin::Uname));
        assert_eq!(builtin("?"), Some(Builtin::Help));
        assert_eq!(builtin("definitely-not-a-command"), None);
    }
}
