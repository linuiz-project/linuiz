bitflags::bitflags! {
    pub struct CR3Flags : u64 {
        const PAGE_LEVEL_WRITE_THROUGH = 1 << 3;
        const PAGE_LEVEL_CACHE_DISABLE = 1 << 4;
    }
}

pub struct CR3;

impl CR3 {
    pub unsafe fn write(frame: &crate::memory::Frame, flags: CR3Flags) {
        asm!("mov cr3, {}", in(reg) frame.addr_u64() | flags.bits(), options(nostack));
    }

    pub fn read() -> CR3Flags {
        let value: u64;

        unsafe {
            asm!("mov {}, cr3", out(reg) value, options(nostack));
        }

        CR3Flags::from_bits_truncate(value)
    }

    pub fn refresh() {
        let value: u64;

        unsafe {
            asm!("mov {0}, cr3", out(reg) value, options(nostack));
            asm!("mov cr3, {0}", in(reg) value, options(nostack));
        }
    }
}
