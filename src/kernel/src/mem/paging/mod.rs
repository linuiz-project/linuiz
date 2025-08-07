use crate::mem::{
    HigherHalfDirectMap,
    paging::page_table::{Depth, Entry},
};
use core::ops::ControlFlow;
use libsys::{
    address::{Address, Frame, Page},
    constants::table_index_size,
};

pub mod page_table;
use page_table::PageTable;
pub use page_table::{CreateEntryError, WithEntryError};

/// Whether the current environment supports 2MiB pages.
pub fn use_large_pages() -> bool {
    cfg_select! {
        target_arch = "x86_64" => {
            use crate::arch::x86_64::{cpuid::feature_info, registers::control::cr4};
            use raw_cpuid::FeatureInfo;

            debug_assert!(
                feature_info().is_some_and(FeatureInfo::has_pae)
                    && cr4::CR4::read().contains(cr4::Flags::PAE)
            );

            true
        }

        _ => { todo!() }
    }
}

/// Whether the current environment supports 1GiB pages.
pub fn use_huge_pages() -> bool {
    cfg_select! {
        target_arch = "x86_64" => {
            use crate::arch::x86_64::cpuid::extended_feature_identifiers;
            use raw_cpuid::ExtendedProcessorFeatureIdentifiers;

            extended_feature_identifiers()
                .is_some_and(ExtendedProcessorFeatureIdentifiers::has_1gib_pages)
        }

        _ => { todo!() }
    }
}

#[derive(Debug, Clone)]
pub struct RootTable(PageTable);

impl RootTable {
    pub const fn empty() -> Self {
        Self(PageTable::empty())
    }

    /// Returns the page address of this table.
    pub fn page(&self) -> Address<Page> {
        let self_ptr = core::ptr::from_ref(&self.0).cast_mut();
        Address::<Page>::try_from(self_ptr).expect("`&self` was not page-aligned")
    }

    /// Returns the frame address of this table.
    pub fn frame(&self) -> Address<Frame> {
        HigherHalfDirectMap::page_to_frame(self.page())
    }

    pub fn with_entry<T>(
        &self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&Entry) -> T,
    ) -> Result<T, WithEntryError> {
        // Safety: This is the start of page table traversal, so current depth is
        // `Depth::min()`.
        unsafe { self.0.with_entry(page, Depth::min(), to_depth, with_fn) }
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, WithEntryError> {
        // Safety: This is the start of page table traversal, so current depth is
        // `Depth::min()`.
        unsafe { self.0.with_entry_mut(page, Depth::min(), to_depth, with_fn) }
    }

    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Depth,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, CreateEntryError> {
        self.0
            .with_entry_create(page, Depth::min(), to_depth, with_fn)
    }

    pub fn walk<E>(
        &self,
        mut func: impl FnMut(Option<(Depth, &Entry)>) -> ControlFlow<E>,
    ) -> ControlFlow<E> {
        #[allow(unreachable_code, unused_variables)]
        fn walk_impl<'a, E>(
            page_table: &'a PageTable,
            current_depth: Depth,
            to_depth: Depth,
            func: &mut impl FnMut(Option<(Depth, &'a Entry)>) -> ControlFlow<E>,
        ) -> ControlFlow<E> {
            todo!(
                "I think this function is actually broken. It doesn't traverse the address space correctly if huge pages are enabled."
            );

            page_table.iter().try_for_each(|entry| {
                let is_entry_intermediate = {
                    cfg_select! {
                        target_arch = "x86_64" => {
                            entry.is_huge() || current_depth == Depth::max()
                        }

                        _ => { todo!() }
                    }
                };

                if is_entry_intermediate {
                    func(Some((current_depth, entry)))
                } else if let Some(next_page_table) = entry.page_table() {
                    walk_impl(next_page_table, current_depth.next(), to_depth, func)
                } else {
                    let (steps, _) = core::iter::Step::steps_between(&current_depth, &to_depth);
                    let iterations = table_index_size().pow(u32::try_from(steps).unwrap());
                    (0..iterations).try_for_each(|_| func(None))
                }
            })
        }

        walk_impl(&self.0, Depth::min(), Depth::max(), &mut func)
    }
}

// pub fn with_entry<T>(
//     &self,
//     page: Address<Page>,
//     to_depth: Option<TableDepth>,
//     with_fn: impl FnOnce(&PageTableEntry) -> T,
// ) -> Result<T, Error> {
//     if self.depth() == to_depth.unwrap_or(TableDepth::max()) {
//         Ok(with_fn(self.entry))
//     } else if !self.is_huge() {
//         let next_depth = self.depth().next_checked().unwrap();
//         let entry_index = self.depth().index_of(page.get()).unwrap();
//         let sub_entry = self.entries().get(entry_index).unwrap();

//         if sub_entry.is_present() {
//             // Safety: Since the state of the page tables can not be fully
// modelled or controlled within the kernel itself,             //          we
// can't be 100% certain this is safe. However, in the case that it isn't,
// there's a near-certain             //          chance that the entire kernel
// will explode shortly after reading bad data like this as a page table.
//             (unsafe { PageTable::<Ref>::new(next_depth, sub_entry) })
//                 .with_entry(page, to_depth, with_fn)
//         } else {
//             Err(Error::NotMapped(page.get()))
//         }
//     } else {
//         Err(Error::HugePageEncountered)
//     }
// }

// pub fn with_entry_mut<T>(
//     &mut self,
//     page: Address<Page>,
//     to_depth: Option<TableDepth>,
//     with_fn: impl FnOnce(&mut PageTableEntry) -> T,
// ) -> Result<T, Error> {
//     if self.depth() == to_depth.unwrap_or(TableDepth::max()) {
//         Ok(with_fn(self.entry))
//     } else if !self.is_huge() {
//         let next_depth = self.depth().next_checked().unwrap();
//         let entry_index = self.depth().index_of(page.get()).unwrap();
//         let sub_entry = self.entries_mut().get_mut(entry_index).unwrap();

//         if sub_entry.is_present() {
//             // Safety: Since the state of the page tables can not be fully
// modelled or controlled within the kernel itself,             //          we
// can't be 100% certain this is safe. However, in the case that it isn't,
// there's a near-certain             //          chance that the entire kernel
// will explode shortly after reading bad data like this as a page table.
//             (unsafe { PageTable::<Mut>::new(next_depth, sub_entry) })
//                 .with_entry_mut(page, to_depth, with_fn)
//         } else {
//             Err(Error::NotMapped(page.get()))
//         }
//     } else {
//         Err(Error::HugePageEncountered)
//     }
// }

// /// Attempts to get a mutable reference to the page table that lies in the
// given entry index's frame, or /// creates the sub page table if it doesn't
// exist. This function returns `None` if it was unable to allocate /// a frame
// for the requested page table. pub fn with_entry_create<T>(
//     &mut self,
//     page: Address<Page>,
//     to_depth: TableDepth,
//     with_fn: impl FnOnce(&mut PageTableEntry) -> T,
// ) -> Result<T, Error> {
//     if self.depth() == to_depth {
//         Ok(with_fn(self.entry))
//     } else if !self.is_huge() {
//         if !self.is_present() {
//             debug_assert!(
//                 self.get_frame() == Address::default(),
//                 "page table entry is non-present, but has a present frame
// address: {:?} {:?}",                 self.depth(),
//                 self.entry
//             );

//             trace!(
//                 "Creating table: {page:X?} (to_depth:{}, current:{})",
//                 to_depth.get(),
//                 self.depth().get()
//             );

//             let mut flags = TableEntryFlags::PTE;
//             // Insert the USER bit in all non-leaf entries.
//             // This is for compatibility with the x86 paging scheme.
//             if !self.depth().is_max() {
//                 flags.insert(TableEntryFlags::USER);
//             }

//             // Set the entry frame and set attributes to make a valid PTE.
//             *self.entry = PageTableEntry::new(
//                 PhysicalMemoryManager::next_frame().map_err(|_|
// Error::AllocError)?,                 flags,
//             );

//             // Clear the table to avoid corrupted PTEs.
//             self.entries_mut().fill(PageTableEntry::empty());
//         }

//         let next_depth = self.depth().next_checked().unwrap();
//         let entry_index = self.depth().index_of(page.get()).unwrap();
//         let sub_entry = self.entries_mut().get_mut(entry_index).unwrap();

//         // Safety: If the page table entry is present, then it's a valid
// entry, all bits accounted.         (unsafe {
// PageTable::<Mut>::new(next_depth, sub_entry) })
// .with_entry_create(page, to_depth, with_fn)     } else {
//         Err(Error::HugePageEncountered)
//     }
// }
