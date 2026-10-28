#![no_std]

//! The uefios shell.
//!
//! This crate is deliberately target-agnostic (`no_std` + `alloc` only) so the
//! parser, builtins, and the `Shell` REPL can be unit-tested on the host. The
//! `os` crate owns the terminal window and event loop.
//!
//! Long-term direction: a POSIX-ish command environment with FreeBSD-derived
//! userland (`sh`/Almquist shell, coreutils) over a libc shim — see
//! docs/ARCHITECTURE.md. `Builtin` is the seed of that surface.

extern crate alloc;

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use alloc::format;

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
    HvTest,
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
        "hvtest" => Builtin::HvTest,
        _ => return None,
    })
}

/// Keys the REPL understands, translated by the caller from raw input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellKey {
    Char(char),
    Enter,
    Backspace,
    HistoryUp,
    HistoryDown,
}

/// Side effects the caller must perform on the shell's behalf.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellAction {
    None,
    Clear,
    Reboot,
    /// Caller should run the VT-x smoke test (`hv::launch`) and feed the
    /// result back through [`Shell::output`].
    HvTest,
}

/// Stateful shell: scrollback + edit line + history.
pub struct Shell {
    /// Completed output lines (scrollback).
    pub out: Vec<String>,
    /// Current input line being edited.
    pub line: String,
    history: Vec<String>,
    hidx: Option<usize>,
    env_info: String,
    /// Bumped whenever visible state changes — the terminal repaints on change.
    pub revision: u64,
}

impl Shell {
    pub fn new(env_info: String) -> Self {
        let mut s = Self {
            out: Vec::new(),
            line: String::new(),
            history: Vec::new(),
            hidx: None,
            env_info,
            revision: 0,
        };
        s.print(&format!(
            "uefios {} -- type 'help' for commands",
            env!("CARGO_PKG_VERSION")
        ));
        s
    }

    /// Feed one key; returns any side-effect the caller must perform.
    pub fn feed(&mut self, key: ShellKey) -> ShellAction {
        self.revision += 1;
        match key {
            ShellKey::Char(c) => {
                self.hidx = None;
                self.line.push(c);
            }
            ShellKey::Backspace => {
                self.hidx = None;
                self.line.pop();
            }
            ShellKey::HistoryUp => {
                let n = self.history.len();
                if n > 0 {
                    let i = self.hidx.map_or(n - 1, |i| i.saturating_sub(1));
                    self.hidx = Some(i);
                    self.line = self.history[i].clone();
                }
            }
            ShellKey::HistoryDown => match self.hidx {
                Some(i) if i + 1 < self.history.len() => {
                    self.hidx = Some(i + 1);
                    self.line = self.history[i + 1].clone();
                }
                _ => {
                    self.hidx = None;
                    self.line.clear();
                }
            },
            ShellKey::Enter => {
                let line = core::mem::take(&mut self.line);
                self.hidx = None;
                self.print(&format!("> {}", line));
                if !line.trim().is_empty() {
                    self.history.push(line.clone());
                    return self.exec(&line);
                }
            }
        }
        ShellAction::None
    }

    fn print(&mut self, s: &str) {
        for l in s.split('\n') {
            self.out.push(l.to_string());
        }
    }

    /// Append externally-produced output (e.g. an `ShellAction`'s result)
    /// to the scrollback and bump the revision.
    pub fn output(&mut self, s: &str) {
        self.revision += 1;
        self.print(s);
    }

    /// Lines to display: scrollback tail + the live prompt line.
    pub fn render(&self, rows: usize, dst: &mut Vec<String>) {
        dst.clear();
        let start = self.out.len().saturating_sub(rows.saturating_sub(1));
        dst.extend(self.out[start..].iter().cloned());
        dst.push(format!("> {}\u{2588}", self.line));
        while dst.len() > rows {
            dst.remove(0);
        }
    }

    fn exec(&mut self, line: &str) -> ShellAction {
        let Some(parsed) = parse(line) else { return ShellAction::None };
        let name = parsed.name().unwrap_or("");
        match builtin(name) {
            Some(Builtin::Help) => self.print(
                "commands: help ? ver uname clear efiinfo echo hvtest exit\n\
                 built-ins only for now -- userland lands in phase 4",
            ),
            Some(Builtin::Version) => self.print(&format!(
                "uefios {} (kernelless, UEFI boot services)",
                env!("CARGO_PKG_VERSION")
            )),
            Some(Builtin::Uname) => self.print("uefios x86_64 uefi"),
            Some(Builtin::EfiInfo) => self.print(&self.env_info.clone()),
            Some(Builtin::Clear) => return ShellAction::Clear,
            Some(Builtin::Exit) => {
                self.print("(the shell runs inside a window -- close it with the X button)")
            }
            Some(Builtin::HvTest) => return ShellAction::HvTest,
            None if name == "echo" => {
                let text = parsed.argv[1..].join(" ");
                self.print(&text);
            }
            None => self.print(&format!("{}: command not found", name)),
        }
        ShellAction::None
    }
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

    #[test]
    fn shell_runs_commands() {
        let mut s = Shell::new("fw".into());
        for c in "uname".chars() {
            s.feed(ShellKey::Char(c));
        }
        s.feed(ShellKey::Enter);
        assert!(s.out.iter().any(|l| l.contains("uefios x86_64")));
    }

    #[test]
    fn shell_history_recall() {
        let mut s = Shell::new("fw".into());
        for c in "ver".chars() {
            s.feed(ShellKey::Char(c));
        }
        s.feed(ShellKey::Enter);
        s.feed(ShellKey::HistoryUp);
        assert_eq!(s.line, "ver");
        s.feed(ShellKey::HistoryDown);
        assert_eq!(s.line, "");
    }

    #[test]
    fn shell_clear_action() {
        let mut s = Shell::new("fw".into());
        for c in "clear".chars() {
            s.feed(ShellKey::Char(c));
        }
        assert_eq!(s.feed(ShellKey::Enter), ShellAction::Clear);
    }
}
