#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
#[cfg(target_arch = "x86_64")]
mod x86_64 {
    use core::num::NonZeroU32;

    pub const fn page_shift() -> NonZeroU32 {
        NonZeroU32::new(12).unwrap()
    }

    pub const fn page_size() -> usize {
        1 << page_shift().get()
    }

    pub const fn page_mask() -> usize {
        page_size().checked_sub(1).unwrap()
    }

    pub const fn table_index_shift() -> NonZeroU32 {
        NonZeroU32::new(9).unwrap()
    }

    pub const fn table_index_size() -> usize {
        1 << table_index_shift().get()
    }

    pub const fn table_index_mask() -> usize {
        table_index_size().checked_sub(1).unwrap()
    }

    pub const fn phys_canonical_mask() -> usize {
        0x000F_FFFF_FFFF_FFFF
    }

    pub const fn checked_phys_canonical(address: usize) -> bool {
        (address & !phys_canonical_mask()) == 0
    }

    #[inline]
    fn paging_depth() -> u32 {
        const CR4_LA57_BIT: usize = 1 << 12;

        let cr4: usize;
        unsafe { core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, pure)) };

        if (cr4 & CR4_LA57_BIT) == 0 { 4 } else { 5 }
    }

    pub fn virt_noncanonical_shift() -> NonZeroU32 {
        let table_indexes_shift = table_index_shift().get() * paging_depth();
        let total_shift = table_indexes_shift + page_shift().get();

        NonZeroU32::new(total_shift).unwrap()
    }

    pub fn checked_virt_canonical(address: usize) -> bool {
        let sign_extension_check_shift = virt_noncanonical_shift().get().checked_sub(1).unwrap();
        matches!(address >> sign_extension_check_shift, 0 | 0x1ffff)
    }
}
