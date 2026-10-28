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
