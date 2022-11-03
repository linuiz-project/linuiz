bitflags::bitflags! {
    pub struct CR0Flags : u64 {
        const PE = 1 << 0;
        const MP = 1 << 1;
        const EM = 1 << 2;
        const TS = 1 << 3;
        const ET = 1 << 4;
        const NE = 1 << 5;
        const WP = 1 << 16;
        const AM = 1 << 18;
        const NW = 1 << 29;
        const CD = 1 << 30;
        const PG = 1 << 31;
    }
}

pub struct CR0;

impl CR0 {
    #[inline]
    pub fn read() -> CR0Flags {
        let value: u64;

        unsafe {
            core::arch::asm!(
                "mov {}, cr0",
                out(reg) value,
                options(nostack, nomem, preserves_flags)
            );
        }

        unsafe { CR0Flags::from_bits_unchecked(value) }
    }

    #[inline]
    pub unsafe fn write(value: CR0Flags) {
        core::arch::asm!(
            "mov cr0, {}",
            in(reg) value.bits(),
            options(nostack, nomem, preserves_flags)
        );
    }

    #[inline]
    pub unsafe fn enable(flags: CR0Flags) {
        let mut new_flags = CR0::read();
        new_flags.set(flags, true);

        CR0::write(new_flags);
    }

    #[inline]
    pub unsafe fn disable(flags: CR0Flags) {
        let mut new_flags = CR0::read();
        new_flags.set(flags, false);

        CR0::write(new_flags);
    }
}
