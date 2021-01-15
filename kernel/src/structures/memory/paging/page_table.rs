use crate::structures::memory::{
    global_allocator_mut,
    paging::{PageAttributes, PageTableEntry},
    Frame,
};
use core::{
    marker::PhantomData,
    ops::{Index, IndexMut},
};

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
    pub fn sub_table_mut(&mut self, index: usize) -> &mut PageTable<L::NextLevel> {
        let entry = &mut self.entries[index];

        let frame = match entry.frame() {
            Some(frame) => frame,
            None => {
                let alloc_frame = global_allocator_mut(|allocator| allocator.alloc_next())
                    .expect("failed to allocate a frame for new page table");
                entry.set(
                    &alloc_frame,
                    PageAttributes::PRESENT | PageAttributes::WRITABLE,
                );

                // TODO map frame with a vaddr, and clear frame somehowa
                alloc_frame
            }
        };

        unsafe {
            // TODO this fails when memory isn't identity-mapped
            &mut *(frame.addr().as_u64() as *mut PageTable<L::NextLevel>)
        }
    }

    pub fn sub_table_frame(&self, index: usize) -> Option<Frame> {
        self.entries[index].frame()
    }

    pub fn frame(&self) -> Frame {
        Frame::from_addr(self.entries.as_ptr() as u64)
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
