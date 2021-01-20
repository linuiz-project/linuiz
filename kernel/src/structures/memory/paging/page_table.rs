use crate::structures::memory::paging::PageTableEntry;
use core::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};
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
}

impl<L> PageTable<L>
where
    L: HeirarchicalLevel,
{
    pub fn sub_table_create(
        &mut self,
        index: usize,
        offset: VirtAddr,
    ) -> &mut PageTable<L::NextLevel> {
        trace!("Accessing sub-table: index {}, offset {:?}", index, offset);
        let entry = &mut self[index];
        let mapped_physical_addr = offset + entry.frame_alloc().addr().as_u64();
        unsafe { &mut *mapped_physical_addr.as_mut_ptr() }
    }
}

impl<L: TableLevel> Index<usize> for PageTable<L> {
    type Output = PageTableEntry;
    fn index(&self, index: usize) -> &Self::Output {
        &self.entries[index]
    }
}

impl<L: TableLevel> IndexMut<usize> for PageTable<L> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.entries[index]
    }
}
