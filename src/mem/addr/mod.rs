pub mod phys;
pub mod virt;

/// Error indicating that attempting to convert a raw address found
/// non-canonical bits.
#[derive(Error, PartialEq, Eq)]
#[error("attempt to convert a raw address found non-canonical bits")]
pub enum NonCanonicalError {
    Address(usize),
    Index { align: usize, index: usize },
    PositiveOffset { base: usize, offset: usize },
    NegativeOffset { base: usize, offset: usize },
}

impl core::fmt::Debug for NonCanonicalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_tuple("NonCanonicalError");

        match self {
            NonCanonicalError::Address(non_canonical_address) => {
                d.field(&format_args!("{non_canonical_address:#X}"));
            }

            NonCanonicalError::Index { align, index } => {
                d.field(&format_args!("{align:#X} * {index:#X}"));
            }

            NonCanonicalError::PositiveOffset { base, offset } => {
                d.field(&format_args!("{base:#X} + {offset:#X}"));
            }

            NonCanonicalError::NegativeOffset { base, offset } => {
                d.field(&format_args!("{base:#X} - {offset:#X}"));
            }
        }

        d.finish()
    }
}
