use core::arch::asm;

bitflags::bitflags! {
    // Wrapper type for `sstatus` register.
    #[repr(transparent)]
    pub struct SSTATUS : u64 {
        const SIE = 1 << 1;
    }
}

impl SSTATUS {
    #[inline(always)]
    pub unsafe fn write(value: Self) {
        asm!("csrwi sstatus, {}", in(reg) value.bits(), options(nostack, nomem));
    }

    pub fn read() -> Self {
        let bits: u64;

        unsafe { asm!("csrr {}, sstatus", out(reg) bits, options(nostack, nomem)) };

        Self::from_bits_truncate(bits)
    }

    #[inline(always)]
    pub unsafe fn set_bits(bits: Self) {
        asm!("csrs sstatus, {}", in(reg) bits.bits(), options(nostack, nomem));
    }

    #[inline(always)]
    pub unsafe fn clear_bits(bits: Self) {
        asm!("csrc sstatus, {}", in(reg) bits.bits(), options(nostack, nomem));
    }

    #[inline(always)]
    pub fn get_bits(bits: Self) -> bool {
        let value: u64;

        unsafe { asm!("csrr {}, sstatus", out(reg) value, options(nostack, nomem)) };

        Self::from_bits_truncate(value).contains(bits)
    }
}

pub mod sstatus {
    use core::arch::asm;

    pub fn get_sie() -> bool {
        let value: u64;

        asm!("csrr {}, sstatus", out(reg) value, options(nostack, nomem));

        (value & 2) > 0
    }

    pub fn set_sie(value: bool) {
        if value {
            asm!("csrsi sstatus, 2", options(nostack, nomem));
        } else {
            asm!("csrci sstatus, 2", options(nostack, nomem));
        }
    }
}

pub mod stvec {
    use core::arch::asm;

    fn read() -> u64 {
        let value: u64;

        unsafe { asm!("csrr {}, stvec", out(reg) value, options(nostack, nomem)) };

        value
    }
}

pub mod satp {
    ///! Wrapper module for the `satp` control register.
    use bit_field::BitField;
    use num_enum::TryFromPrimitive;

    #[repr(u8)]
    #[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
    pub enum Mode {
        Bare = 0,
        Sv39 = 8,
        Sv48 = 9,
        Sv57 = 10,

        /// This is a reserved value.
        Sv64 = 11,
    }

    /// Reads the raw value from the `satp` control register.
    #[inline]
    fn read_raw() -> u64 {
        let value: u64;

        unsafe { core::arch::asm!("csrr {}, satp", out(reg) value, options(nostack, nomem)) };

        value
    }

    /// Writes a raw value to the `satp` control register.
    #[inline]
    fn write_raw(value: u64) {
        unsafe { core::arch::asm!("csrw satp, {}", in(reg) value, options(nostack, nomem)) };
    }

    /// Gets the physical page number from the `satp` control register.
    #[inline]
    pub fn get_ppn() -> usize {
        read_raw().get_bits(0..44) as usize
    }

    static ASID_LEN: spin::Once<usize> = spin::Once::new();

    /// Returns the maximum bit length of ASID.
    pub fn get_asid_len() -> usize {
        *ASID_LEN.call_once(|| {
            let orig_asid = read_raw();
            // Set all ASID bits.
            write_raw(orig_asid | ((u16::MAX as u64) << 44));
            // Return max ASID bit length.
            let bit_len = read_raw().get_bits(44..60).count_ones() as usize;
            // Reset ASID value.
            write_raw(orig_asid);

            bit_len
        })
    }

    /// Gets the ASID from the `satp` control register.
    #[inline]
    pub fn get_asid() -> usize {
        read_raw().get_bits(44..(44 + get_asid_len())) as usize
    }

    /// Gets the current paging level mode from the `satp` control register.
    #[inline]
    pub fn get_mode() -> Mode {
        Mode::try_from(read_raw().get_bits(60..64) as u8).unwrap()
    }

    pub unsafe fn write(
        ppn: usize, /* TODO make this a struct to ensure validity within the bit range */
        asid: u16,
        mode: Mode,
    ) {
        write_raw((ppn as u64) | ((asid as u64) << 44) | ((mode as u64) << 60));
    }
}
