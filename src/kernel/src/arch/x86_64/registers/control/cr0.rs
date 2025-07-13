bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CR0Flags: u64 {
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
    pub fn read() -> CR0Flags {
        let value: u64;

        // Safety: Reading CR0 has no side effects.
        unsafe {
            core::arch::asm!(
                "mov {}, cr0",
                out(reg) value,
                options(nostack, nomem, preserves_flags)
            );
        }

        CR0Flags::from_bits_truncate(value)
    }

    /// # Safety
    ///
    /// TODO
    pub unsafe fn write(value: CR0Flags) {
        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            core::arch::asm!(
                "mov cr0, {}",
                in(reg) value.bits(),
                options(nostack, nomem, preserves_flags)
            );
        }
    }

    pub unsafe fn enable(flags: CR0Flags) {
        let mut new_flags = CR0::read();
        new_flags.set(flags, true);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR0::write(new_flags);
        }
    }

    pub unsafe fn disable(flags: CR0Flags) {
        let mut new_flags = CR0::read();
        new_flags.set(flags, false);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR0::write(new_flags);
        }
    }
}
