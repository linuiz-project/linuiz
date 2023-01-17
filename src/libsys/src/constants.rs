pub use x86_64::*;
mod x86_64 {
    use core::num::NonZeroU32;

    pub const PAGE_SHIFT: NonZeroU32 = NonZeroU32::new(12).unwrap();
    pub const PAGE_SIZE: usize = 1 << PAGE_SHIFT.get();
    pub const PAGE_MASK: usize = PAGE_SIZE.checked_sub(1).unwrap();

    pub const TABLE_INDEX_SHIFT: NonZeroU32 = NonZeroU32::new(9).unwrap();
    pub const TABLE_INDEX_SIZE: usize = 1 << TABLE_INDEX_SHIFT.get();
    pub const TABLE_INDEX_MASK: usize = TABLE_INDEX_SIZE.checked_sub(1).unwrap();

    pub const PHYS_NON_CANONICAL_MASK: usize = 0xFFF0_0000_0000_0000;

    pub const fn checked_phys_canonical(address: usize) -> bool {
        (address & PHYS_NON_CANONICAL_MASK) == 0
    }

    #[inline]
    pub fn virt_canonical_shift() -> NonZeroU32 {
        const CR4_LA57_BIT: usize = 1 << 12;

        // Safety: `asm!` is used safely, and `NonZeroU32` is guaranteed >0.
        unsafe {
            let cr4: usize;
            core::arch::asm!("mov {}, cr4", out(reg) cr4, options(nomem, pure));

            let paging_depth = if (cr4 & CR4_LA57_BIT) > 0 { 3 } else { 4 };
            NonZeroU32::new_unchecked((TABLE_INDEX_SHIFT.get() * paging_depth) + PAGE_SHIFT.get())
        }
    }

    #[inline]
    pub fn virt_noncanonical_mask() -> usize {
        let shift = virt_canonical_shift().get();
        usize::MAX >> shift << shift
    }

    pub fn checked_virt_canonical(address: usize) -> bool {
        let canonical_extension_bits = virt_noncanonical_mask();
        let extension_bits = address >> virt_canonical_shift().get();

        extension_bits == 0 || extension_bits == canonical_extension_bits
    }
}
