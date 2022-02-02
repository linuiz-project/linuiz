mod header;
mod section_header;
mod segment_header;

pub use header::*;
pub use section_header::*;
pub use segment_header::*;

use crate::{Virtual, Address};

pub const X86_64_RELATIVE: u64 = 0x00000008;

#[derive(Debug)]
pub struct Rela64 {
    pub addr: Address<Virtual>,
    pub info: u64,
    pub addend: u64,
}
