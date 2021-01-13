use crate::structures::memory::{
    paging::{global_allocator, PageAttributes, PageTableEntry},
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
            .for_each(|descriptor| descriptor.set_unused());
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

        if !entry.attribs().contains(PageAttributes::PRESENT) {
            match global_allocator(|allocator| allocator.allocate_next()) {
                Some(mut frame) => {
                    entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);

                    unsafe {
                        frame.clear();
                    }
                }
                None => panic!("failed to lock a frame for new page table"),
            }
        }

        unsafe {
            &mut *(entry
                .frame()
                .expect("descriptor has no valid frame")
                .addr()
                .as_u64() as *mut PageTable<L::NextLevel>)
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

// impl<L> core::fmt::Debug for PageTable<dyn TableLevel> {
//     fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
//         let mut debug_struct = formatter.debug_struct("Page Table");

//         for entry in self.iter() {
//             debug_struct.field("Entry", entry);
//         }

//         debug_struct.finish()
//     }
// }
