mod entry;
mod table;

pub use entry::*;

const ENTRY_COUNT: usize = 512;

pub struct Page {
    number: usize,
}
