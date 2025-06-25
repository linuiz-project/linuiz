bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct CR4Flags: u64 {
        const VME           = 1 << 0;
        const PVI           = 1 << 1;
        const TSD           = 1 << 2;
        const DE            = 1 << 3;
        const PSE           = 1 << 4;
        const PAE           = 1 << 5;
        const MCE           = 1 << 6;
        const PGE           = 1 << 7;
        const PCE           = 1 << 8;
        const OSFXSR        = 1 << 9;
        const OSXMMEXCPT    = 1 << 10;
        const UMIP          = 1 << 11;
        const LA57          = 1 << 12;
        const VMXE          = 1 << 13;
        const SMXE          = 1 << 14;
        const FSGSBASE      = 1 << 16;
        const PCIDE         = 1 << 17;
        const OSXSAVE       = 1 << 18;
        const SMEP          = 1 << 20;
        const SMAP          = 1 << 21;
        const PKE           = 1 << 22;
        const CET           = 1 << 23;
        const PKS           = 1 << 24;
    }
}

pub struct CR4;

impl CR4 {
    #[inline]
    pub fn read() -> CR4Flags {
        let value: u64;

        // Safety: Reading into a register has no side effects.
        unsafe {
            core::arch::asm!(
                "mov {}, cr4",
                out(reg) value,
                options(nostack, nomem)
            );
        }

        CR4Flags::from_bits_truncate(value)
    }

    /// ## Safety
    ///
    /// - `flags` must contain only features supported by the current CPU.
    /// - `flags` must not be updated at such a point as the features in question are in use.
    #[inline]
    pub unsafe fn write(flags: CR4Flags) {
        // Safety: Caller is required to ensure updating `CR4` with the provided flags will not cause undefined behaviour.
        unsafe {
            core::arch::asm!(
                "mov cr4, {}",
                in(reg) flags.bits(),
                options(nostack, nomem)
            );
        }
    }

    /// ## Safety
    ///
    /// Incorrect flags may violate any number of safety guarantees.
    #[inline]
    pub unsafe fn enable(flags: CR4Flags) {
        let mut new_flags = CR4::read();
        new_flags.set(flags, true);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR4::write(new_flags);
        }
    }

    /// # Safety
    ///
    /// Incorrect flags may violate any number of safety guarantees.
    #[inline]
    pub unsafe fn disable(flags: CR4Flags) {
        let mut new_flags = CR4::read();
        new_flags.set(flags, false);

        // Safety: Caller is required to maintain safety invariants.
        unsafe {
            CR4::write(new_flags);
        }
    }
}
