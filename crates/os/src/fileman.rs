//! File manager window state: browses the ESP (fs0) via the UEFI
//! Simple File System and launches .efi images with LoadImage/StartImage —
//! our "container v0" process primitive (child takes the console until it exits).

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use uefi::fs::{FileSystem, PathBuf};
use uefi::{boot, CString16};
use shell::ShellKey;

pub struct FmEntry {
    pub name: String,
    pub is_dir: bool,
}

pub struct FileManager {
    pub fs: FileSystem,
    /// Directory as components under the volume root, e.g. ["EFI","BOOT"].
    pub cwd: Vec<String>,
    pub entries: Vec<FmEntry>,
    pub sel: usize,
    /// Bumped whenever content must be re-rendered.
    pub revision: u64,
    pub msg: String,
    /// Set by Enter on a .efi file; the event loop performs the launch.
    pub pending_launch: Option<String>,
}

impl FileManager {
    pub fn new(fs: FileSystem) -> Self {
        let mut fm = Self {
            fs,
            cwd: Vec::new(),
            entries: Vec::new(),
            sel: 0,
            revision: 0,
            msg: String::new(),
            pending_launch: None,
        };
        fm.reload();
        fm
    }

    pub fn path_str(&self) -> String {
        if self.cwd.is_empty() {
            String::from("fs0:\\")
        } else {
            alloc::format!("fs0:\\{}", self.cwd.join("\\"))
        }
    }

    fn pathbuf(&self) -> PathBuf {
        let s = if self.cwd.is_empty() {
            String::from("\\")
        } else {
            alloc::format!("\\{}", self.cwd.join("\\"))
        };
        let c = CString16::try_from(s.as_str()).unwrap();
        PathBuf::from(&*c)
    }

    pub fn reload(&mut self) {
        self.entries.clear();
        self.sel = 0;
        match self.fs.read_dir(self.pathbuf()) {
            Ok(iter) => {
                let mut dirs = Vec::new();
                let mut files = Vec::new();
                for e in iter.flatten() {
                    let name = String::from_utf16_lossy(e.file_name().to_u16_slice());
                    if name == "." || name == ".." {
                        continue;
                    }
                    let ent = FmEntry { name, is_dir: e.is_directory() };
                    if ent.is_dir {
                        dirs.push(ent);
                    } else {
                        files.push(ent);
                    }
                }
                dirs.append(&mut files);
                self.entries = dirs;
                self.msg = alloc::format!("{} entries", self.entries.len());
            }
            Err(e) => self.msg = alloc::format!("read_dir failed: {:?}", e),
        }
        self.revision += 1;
    }

    fn enter(&mut self) {
        match self.entries.get(self.sel) {
            Some(FmEntry { is_dir: true, name }) => {
                self.cwd.push(name.clone());
                self.reload();
            }
            Some(FmEntry { name, .. }) => {
                let mut p = self.path_str();
                p.push('\\');
                p.push_str(name);
                if name.to_ascii_lowercase().ends_with(".efi") {
                    self.pending_launch = Some(p);
                } else {
                    self.msg = alloc::format!("{} (not bootable)", name);
                    self.revision += 1;
                }
            }
            None => {}
        }
    }

    fn up_dir(&mut self) {
        if self.cwd.pop().is_some() {
            self.reload();
        }
    }

    /// Arrow keys navigate; Enter opens/launches; Backspace goes up a dir.
    pub fn feed(&mut self, k: ShellKey) {
        match k {
            ShellKey::HistoryUp | ShellKey::Char('k') => {
                if self.sel > 0 { self.sel -= 1; }
                self.revision += 1;
            }
            ShellKey::HistoryDown | ShellKey::Char('j') => {
                if self.sel + 1 < self.entries.len() { self.sel += 1; }
                self.revision += 1;
            }
            ShellKey::Enter => self.enter(),
            ShellKey::Backspace => self.up_dir(),
            _ => {}
        }
    }

    /// Launch the pending .efi path — blocks in StartImage until it exits.
    /// Caller passes the current image handle.
    pub fn launch_pending(&mut self) {
        if let Some(path) = self.pending_launch.take() {
            let s = path.strip_prefix("fs0:").unwrap_or(&path).to_string();
            let c = match CString16::try_from(s.as_str()) {
                Ok(c) => c,
                Err(_) => { self.msg = "bad path".into(); self.revision += 1; return; }
            };
            match self.fs.read(PathBuf::from(&*c)) {
                Ok(img) => {
                    let ih = boot::image_handle();
                    match boot::load_image(
                        ih,
                        boot::LoadImageSource::FromBuffer { buffer: &img, file_path: None },
                    ) {
                        Ok(child) => {
                            let st = boot::start_image(child)
                                .map(|_| uefi::Status::SUCCESS)
                                .unwrap_or_else(|e| e.status());
                            self.msg = alloc::format!("child exited: {:?}", st);
                        }
                        Err(e) => self.msg = alloc::format!("LoadImage: {:?}", e.status()),
                    }
                }
                Err(e) => self.msg = alloc::format!("read: {:?}", e),
            }
            self.reload();
        }
    }
}
