use crate::{addr_ty::Virtual, memory::paging::PageTableEntry, Address};
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
            entries: [PageTableEntry::unused(); 512],
            level: PhantomData,
        }
    }

    pub unsafe fn clear(&mut self) {
        self.entries.iter_mut().for_each(|entry| entry.set_unused());
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
        phys_mapped_addr: Address<Virtual>,
    ) -> Option<&PageTable<L::NextLevel>> {
        self.get_entry(index)
            .frame()
            .map(|frame| &*(phys_mapped_addr + frame.base_addr().as_usize()).as_ptr())
    }

    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        phys_mapped_addr: Address<Virtual>,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        self.get_entry_mut(index)
            .frame()
            .map(|frame| &mut *(phys_mapped_addr + frame.base_addr().as_usize()).as_mut_ptr())
    }

    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        phys_mapped_addr: Address<Virtual>,
    ) -> &mut PageTable<L::NextLevel> {
        let entry = self.get_entry_mut(index);
        let (frame, created) = match entry.frame() {
            Some(frame) => (frame, false),
            None => {
                let alloc_frame = crate::memory::falloc::get()
                    .lock_next()
                    .expect("failed to allocate a frame for new page table");
                trace!("Allocated frame for nonpresent entry: {:?}", alloc_frame);

                entry.set(
                    &alloc_frame,
                    crate::memory::paging::PageAttributes::PRESENT
                        | crate::memory::paging::PageAttributes::WRITABLE,
                );

                (alloc_frame, true)
            }
        };

        let sub_table: &mut PageTable<L::NextLevel> =
            &mut *(phys_mapped_addr + frame.base_addr().as_usize()).as_mut_ptr();

        if created {
            sub_table.clear();
        }

        sub_table
    }
}
