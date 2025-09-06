pub mod phys;
pub mod virt;

/// Error indicating that attempting to convert a raw address found
/// non-canonical bits.
#[derive(Debug, Error, PartialEq, Eq)]
#[error("attempt to convert a raw address found non-canonical bits")]
pub struct NonCanonicalError;
