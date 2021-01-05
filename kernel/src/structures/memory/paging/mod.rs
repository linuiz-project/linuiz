mod entry;
mod page_frame_allocator;
mod table;

pub use entry::*;
pub use page_frame_allocator::PageFrameAllocator;

const ENTRY_COUNT: usize = 512;

pub struct Page {
    number: usize,
}
