mod frame_allocator;
mod page_descriptor;
mod page_table;
mod page_table_manager;

pub use frame_allocator::FrameAllocator;
pub use page_descriptor::{PageAttributes, PageDescriptor};
pub use page_table::PageTable;
pub use page_table_manager::PageTableManager;

pub struct Page {
    number: usize,
}

impl Page {
    pub fn new(number: usize) -> Self {
        Self { number }
    }
}
