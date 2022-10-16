/// Calls a breakpoint exception.
#[inline(always)]
#[cfg(target_arch = "x86_64")]
pub fn breakpoint() {
    unsafe {
        core::arch::asm!("int3");
    }
}

pub mod tables {
    pub use x86_64::instructions::tables::*;
}

pub mod sync {
    #[inline]
    pub fn mfence() {
        unsafe { core::arch::asm!("mfence", options(nostack, nomem, preserves_flags)) };
    }
}

pub mod tlb {
    use libcommon::{Address, Page};

    /// Invalidates a single page from the TLB.
    #[inline(always)]
    pub fn invlpg(page: Address<Page>) {
        unsafe {
            core::arch::asm!("invlpg [{}]", in(reg) page.address().as_u64(), options(nostack, preserves_flags));
        }
    }

    //     pub mod pcid {

    //         /// Error indicating a given PCID value is outside the 12 maximum allowed bits.
    //         #[repr(transparent)]
    //         #[derive(Debug)]
    //         pub struct OutsidePCIDRange(u64);

    //         /// Process context identifier structure, for use in CR3.PCID to
    //         /// provide a process ID to the TLB for efficient flushing.
    //         #[repr(transparent)]
    //         #[derive(Debug)]
    //         pub struct PCID(u64);

    //         impl PCID {
    //             /// Creates a new PCID, validating the ID fits in `CR3.PCID` (lower 12 bits, 0..4096).
    //             pub const fn new(pcid: u64) -> Result<Self, OutsidePCIDRange> {
    //                 match pcid {
    //                     0..4096 => Ok(Self(pcid)),
    //                     pcid => Err(OutsidePCIDRange(pcid)),
    //                 }
    //             }
    //         }

    //         /// Indicates the type of call when executing the `invlpcid` instruction.
    //         #[repr(u64)]
    //         pub enum InvalidateType {
    //             IndividualAddress = 0,
    //             SingleContext = 1,
    //             AllContextWithGlobal = 2,
    //             AllContextWithoutGloval = 3,
    //         }

    //         /// Descriptor for use when executing the `invlpcid` instruction.
    //         #[repr(C)]
    //         pub struct InvalidateDescriptor {
    //             page: libcommon::memory::Page,
    //             pcid: PCID,
    //         }

    //         /// Invalidates a specific TLB entry(s), using the given type and descriptor to specify which entry(s).
    //         pub fn invpcid(ty: InvalidateType, invl_descriptor: InvalidateDescriptor) {
    //             unsafe {
    //                 core::arch::asm!(
    //                     "invpcid {}, [{}]",
    //                     in(reg) ty as u64,
    //                     in(reg) &raw const invl_descriptor,
    //                     options(nostack, nomem, preserves_flags)
    //                 )
    //             };
    //         }
    //     }
}
