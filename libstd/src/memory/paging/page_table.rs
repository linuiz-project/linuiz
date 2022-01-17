use crate::memory::{paging::PageTableEntry, Page};
use core::marker::PhantomData;

pub trait TableLevel {}

pub enum Level4 {}
pub enum Level3 {}
pub enum Level2 {}
pub enum Level1 {}

impl TableLevel for Level4 {}
impl TableLevel for Level3 {}
impl TableLevel for Level2 {}
impl TableLevel for Level1 {}

pub trait HeirarchicalLevel: TableLevel {
    type NextLevel: TableLevel;
}
impl HeirarchicalLevel for Level4 {
    type NextLevel = Level3;
}
impl HeirarchicalLevel for Level3 {
    type NextLevel = Level2;
}
impl HeirarchicalLevel for Level2 {
    type NextLevel = Level1;
}

#[repr(C, align(0x1000))]
pub struct PageTable<L: TableLevel> {
    entries: [PageTableEntry; 512],
    level: PhantomData<L>,
}

impl<L: TableLevel> PageTable<L> {
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::UNUSED; 512],
            level: PhantomData,
        }
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes(self as *mut _ as *mut u8, 0, 0x1000);
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }

    pub fn iter(&self) -> core::slice::Iter<PageTableEntry> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<PageTableEntry> {
        self.entries.iter_mut()
    }
}

impl<L: HeirarchicalLevel> PageTable<L> {
    pub unsafe fn sub_table(
        &self,
        index: usize,
        phys_mapped_page: Page,
    ) -> Option<&PageTable<L::NextLevel>> {
        self.get_entry(index).get_frame_index().map(|frame_index| {
            phys_mapped_page
                .forward(frame_index)
                .unwrap()
                .as_ptr::<PageTable<L::NextLevel>>()
                .as_ref()
                .unwrap()
        })
    }

    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        phys_mapped_page: Page,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        self.get_entry_mut(index).get_frame_index().map(|frame_index| {
            phys_mapped_page
                .forward(frame_index)
                .unwrap()
                .as_mut_ptr::<PageTable<L::NextLevel>>()
                .as_mut()
                .unwrap()
        })
    }

    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        phys_mapping_page: Page,
    ) -> &mut PageTable<L::NextLevel> {
        let entry = self.get_entry_mut(index);
        let (frame_index, created) = match entry.get_frame_index() {
            Some(frame_index) => (frame_index, false),
            None => {
                let frame_index = crate::memory::falloc::get().lock_next().unwrap();

                entry.set(
                    frame_index,
                    crate::memory::paging::PageAttributes::PRESENT
                        | crate::memory::paging::PageAttributes::WRITABLE,
                );

                (frame_index, true)
            }
        };

        let sub_table: &mut PageTable<L::NextLevel> = phys_mapping_page
            .forward(frame_index)
            .unwrap()
            .as_mut_ptr::<PageTable<L::NextLevel>>()
            .as_mut()
            .unwrap();

        if created {
            sub_table.clear();
        }

        sub_table
    }
}
