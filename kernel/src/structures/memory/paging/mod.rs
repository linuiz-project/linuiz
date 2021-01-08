mod page_descriptor;
mod page_frame_allocator;
mod page_table;
mod page_table_manager;

pub use page_descriptor::{PageAttributes, PageDescriptor};
pub use page_frame_allocator::PageFrameAllocator;
pub use page_table::PageTable;
pub use page_table_manager::PageTableManager;

const ENTRY_COUNT: usize = 512;
const ADDRESS_SHIFT: usize = 12;

pub struct Page {
    number: usize,
}

impl Page {
    pub fn new(number: usize) -> Self {
        Self { number }
    }
}
