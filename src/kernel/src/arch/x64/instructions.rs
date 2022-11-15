/// Calls a breakpoint exception.
#[inline]
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
    #[inline]
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

#[derive(Debug, Clone, Copy)]
pub enum RdRandError {
    NotSupported,
    HardFailure,
}

/// Reads a cryptographically secure, deterministic random number from hardware.
///
/// Return value indicates various success and failure states of the operation.
/// These are detailed as follows:
///     — `None` indicates no support for the `rdrand` instruction.
///     — `Some(Err(_))` indicates `rdrand` has encountered a hard failure, and will not generate
///         anymore valid numbers.
///     — `Some(Ok(_))` returns the successfully generated random number.
pub fn rdrand() -> Result<u64, RdRandError> {
    // Check to ensure the instruction is supported.
    if crate::arch::x64::cpuid::FEATURE_INFO.has_rdrand() {
        // In the case of a hard failure for random number generation, a retry limit is employed
        // to stop software from entering a busy loop due to bad `rdrand` values.
        for _ in 0..22 {
            let result: u64;
            let rflags: u64;

            unsafe {
                core::arch::asm!(
                    "
                    pushfq      # Save original `rflags`
                    rdrand {}
                    pushfq      # Save `rdrand` `rflags`
                    pop {}      # Pop `rflags` into local variable
                    popfq       # Restore original `rflags`
                    ",
                    out(reg) result,
                    out(reg) rflags,
                    options(pure, nomem, preserves_flags)
                );
            }

            // IA32 Software Developer's Manual specifies it is possible (rarely) for `rdrand` to return
            // bad data in the destination register. If this is the case—and additionally if demand for random
            // number generation is too high—the CF bit in `rflags` will not be set, and in the latter case (throughput),
            // zero will be returned in the destination register.
            use crate::arch::x64::registers::RFlags;
            if result > 0 && RFlags::from_bits_truncate(rflags).contains(RFlags::CARRY_FLAG) {
                return Ok(result);
            } else {
                pause();
            }
        }

        Err(RdRandError::HardFailure)
    } else {
        Err(RdRandError::NotSupported)
    }
}
