use crate::memory::paging::PageTableEntry;
use core::marker::PhantomData;
use x86_64::VirtAddr;

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

impl<L> PageTable<L>
where
    L: TableLevel,
{
    pub const fn new() -> Self {
        Self {
            entries: [PageTableEntry::unused(); 512],
            level: PhantomData,
        }
    }

    pub fn clear(&mut self) {
        self.iter_mut()
            .for_each(|descriptor| descriptor.set_nonpresent());
    }

    pub fn iter(&self) -> core::slice::Iter<PageTableEntry> {
        self.entries.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<PageTableEntry> {
        self.entries.iter_mut()
    }

    pub fn get_entry(&self, index: usize) -> &PageTableEntry {
        &self.entries[index]
    }

    pub fn get_entry_mut(&mut self, index: usize) -> &mut PageTableEntry {
        &mut self.entries[index]
    }
}

impl<L> PageTable<L>
where
    L: HeirarchicalLevel,
{
    pub unsafe fn sub_table(
        &self,
        index: usize,
        offset: VirtAddr,
    ) -> Option<&PageTable<L::NextLevel>> {
        trace!(
            "Accessing sub-table (ref): index {}, offset {:?}",
            index,
            offset
        );
        self.get_entry(index)
            .frame()
            .map(|frame| &*(offset + frame.addr_u64()).as_ptr())
    }

    pub unsafe fn sub_table_mut(
        &mut self,
        index: usize,
        offset: VirtAddr,
    ) -> Option<&mut PageTable<L::NextLevel>> {
        trace!(
            "Accessing sub-table (mut): index {}, offset {:?}",
            index,
            offset
        );
        self.get_entry_mut(index)
            .frame()
            .map(|frame| &mut *(offset + frame.addr_u64()).as_mut_ptr())
    }

    pub unsafe fn sub_table_create(
        &mut self,
        index: usize,
        offset: VirtAddr,
    ) -> &mut PageTable<L::NextLevel> {
        trace!(
            "Accessing sub-table (create): index {}, offset {:?}",
            index,
            offset
        );
        let entry = self.get_entry_mut(index);
        let (frame, created) = match entry.frame() {
            Some(frame) => (frame, false),
            None => {
                trace!("Allocating frame for previously nonpresent entry.");
                let alloc_frame = crate::memory::global_memory()
                    .lock_next()
                    .expect("failed to allocate a frame for new page table");

                entry.set(
                    &alloc_frame,
                    crate::memory::paging::PageAttributes::PRESENT
                        | crate::memory::paging::PageAttributes::WRITABLE,
                );

                (alloc_frame, true)
            }
        };

        let table_ptr: &mut PageTable<L::NextLevel> =
            &mut *(offset + frame.addr_u64()).as_mut_ptr();

        if created {
            table_ptr.clear()
        }

        table_ptr
    }
}
