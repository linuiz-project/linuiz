use core::ops::{Index, IndexMut};

use crate::structures::memory::paging::{ENTRY_COUNT, PageEntry, PageEntryFlags};

pub struct PageTable {
    entries: [PageEntry; ENTRY_COUNT]
}

impl PageTable {
    pub fn clear(&mut self) {
        for page_entry in self.entries.iter_mut() {
            page_entry.set_unused();
        }
    }
}

impl Index<usize> for PageTable {
    type Output = PageEntry;

    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl IndexMut<usize> for PageTable {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}