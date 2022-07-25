bitflags::bitflags! {
    pub struct CR4Flags : u64 {
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
    #[inline(always)]
    pub fn read() -> CR4Flags {
        let value: u64;

        unsafe {
            core::arch::asm!(
                "mov {}, cr4",
                out(reg) value,
                options(nostack, nomem)
            );
        }

        unsafe { CR4Flags::from_bits_unchecked(value) }
    }

    #[inline(always)]
    pub unsafe fn write(value: CR4Flags) {
        core::arch::asm!(
            "mov cr4, {}",
            in(reg) value.bits(),
            options(nostack, nomem)
        );
    }

    #[inline(always)]
    pub unsafe fn enable(flags: CR4Flags) {
        let mut new_flags = CR4::read();
        new_flags.set(flags, true);

        CR4::write(new_flags);
    }

    #[inline(always)]
    pub unsafe fn disable(flags: CR4Flags) {
        let mut new_flags = CR4::read();
        new_flags.set(flags, false);

        CR4::write(new_flags);
    }
}
