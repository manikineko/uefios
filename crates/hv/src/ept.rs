//! Extended Page Tables — second-level translation so each guest gets its
//! own physical-address view while host RAM stays mapped identically.
//! Design follows MiniVisor's ExtendedPageTables.c.

/// EPT paging-structure entry bits.
pub const EPT_READ: u64 = 1 << 0;
pub const EPT_WRITE: u64 = 1 << 1;
pub const EPT_EXEC: u64 = 1 << 2;
pub const EPT_MEM_TYPE_WB: u64 = 6 << 3;
pub const EPT_PAT: u64 = 1 << 6;
pub const EPT_SUPERPAGE: u64 = 1 << 7; // 2 MiB PDE
pub const EPT_ACCESSED: u64 = 1 << 8;
pub const EPT_DIRTY: u64 = 1 << 9;

/// Top guest physical address the identity map below covers (4 GiB).
pub const COVERED_GPA: u64 = 4 * 1024 * 1024 * 1024;

/// EPTP value: mem type 6 (WB), walk length 4 -> (4-1).
pub fn eptp(pml4_phys: u64) -> u64 {
    pml4_phys | 6 | (3 << 3)
}

/// A 2 MiB superpage PDE mapping GPA==HPA with full access.
pub const fn superpage(addr: u64) -> u64 {
    (addr & !0x1f_ffff)
        | EPT_READ
        | EPT_WRITE
        | EPT_EXEC
        | EPT_MEM_TYPE_WB
        | EPT_SUPERPAGE
}

/// EPT guest-physical view: identity map for the first 4 GiB using
/// 2 MiB superpages (PML4 + PDPT + 4xPD = six 4 KiB pages).
/// Anything the guest touches — code, stack, GDT — must live below
/// [`COVERED_GPA`].
#[repr(C, align(4096))]
pub struct EptTables {
    pub pml4: [u64; 512],
    pub pdpt: [u64; 512],
    pub pd: [[u64; 512]; 4],
}

impl EptTables {
    /// Number of 4 KiB pages the caller must allocate for this struct.
    pub const PAGES: usize = core::mem::size_of::<Self>() / 4096;

    /// Fill tables so GPA==HPA, full RWX. `base` is this struct's PA.
    pub unsafe fn init_identity(&mut self, base: u64) {
        let pdpt_pa = base + core::mem::size_of::<[u64; 512]>() as u64;
        self.pml4[0] = pdpt_pa | EPT_READ | EPT_WRITE | EPT_EXEC;
        let pd_base = pdpt_pa + core::mem::size_of::<[u64; 512]>() as u64;
        for i in 0..4 {
            self.pdpt[i] =
                pd_base + i as u64 * 4096 | EPT_READ | EPT_WRITE | EPT_EXEC;
            for j in 0..512 {
                let gpa = (i * 512 + j) as u64 * 0x20_0000;
                self.pd[i][j] = superpage(gpa);
            }
        }
    }
}
