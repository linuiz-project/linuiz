use crate::memory::Page;

/// Invalidates a single page from the TLB.
#[inline(always)]
pub fn invlpg(page: &Page) {
    unsafe {
        core::arch::asm!("invlpg [{}]", in(reg) page.index() * 0x1000, options(nostack, preserves_flags));
    }
}

/// Switches the current CR3 register in and out, causing all TLB entries to be invalidated.
#[inline(always)]
pub fn invlpg_all() {
    libarch::registers::x86_64::control::CR3::refresh();
}

pub mod pcid {
    /// Error indicating a given PCID value is outside the 12 maximum allowed bits.
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct OutsidePCIDRange(u64);

    /// Process context identifier structure, for use in CR3.PCID to
    /// provide a process ID to the TLB for efficient flushing.
    #[repr(transparent)]
    #[derive(Debug)]
    pub struct PCID(u64);

    impl PCID {
        /// Creates a new PCID, validating the ID fits in `CR3.PCID` (lower 12 bits, 0..4096).
        pub const fn new(pcid: u64) -> Result<Self, OutsidePCIDRange> {
            match pcid {
                0..4096 => Ok(Self(pcid)),
                pcid => Err(OutsidePCIDRange(pcid)),
            }
        }
    }

    /// Indicates the type of call when executing the `invlpcid` instruction.
    #[repr(u64)]
    pub enum InvalidateType {
        IndividualAddress = 0,
        SingleContext = 1,
        AllContextWithGlobal = 2,
        AllContextWithoutGloval = 3,
    }

    /// Descriptor for use when executing the `invlpcid` instruction.
    #[repr(C)]
    pub struct InvalidateDescriptor {
        page: crate::memory::Page,
        pcid: PCID,
    }

    /// Invalidates a specific TLB entry(s), using the given type and descriptor to specify which entry(s).
    pub fn invpcid(ty: InvalidateType, invl_descriptor: InvalidateDescriptor) {
        unsafe {
            core::arch::asm!(
                "invpcid {}, [{}]",
                in(reg) ty as u64,
                in(reg) &raw const invl_descriptor,
                options(nostack, nomem, preserves_flags)
            )
        };
    }
}
