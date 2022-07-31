use core::arch::asm;

bitflags::bitflags! {
    pub struct CR3Flags : u64 {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(addr: u64, flags: CR3Flags) {
        assert_eq!(addr & 0xFFF, 0, "address must be frame-aligned (low 12 bits empty)");

        asm!("mov cr3, {}", in(reg) addr | flags.bits(), options(nostack));
    }

    pub fn read() -> (u64, CR3Flags) {
        let value: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack, nomem));
        }

        (value & !CR3Flags::all().bits(), CR3Flags::from_bits_truncate(value))
    }

    #[inline(always)]
    pub fn refresh() {
        let value: u64;

        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack, preserves_flags));
            asm!("mov cr3, {0}", in(reg) value, options(nostack, preserves_flags));
        }
    }
}
