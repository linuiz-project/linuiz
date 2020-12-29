mod entry;
mod table;

use crate::structures::memory::Frame;
pub use entry::*;

const ENTRY_COUNT: usize = 512;

pub struct Page {
    number: usize,
}
