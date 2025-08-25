use core::{marker::PhantomData, num::NonZero, ptr::NonNull};

use crate::{
    mem::{
        HigherHalfDirectMap,
        pmm::{NextFrameError, PhysicalMemoryManager},
    },
    util::{ExclusiveBorrow, InteriorBorrow, SharedBorrow},
};
use libsys::{
    address::{Address, Frame, Page},
    constants::table_index_size,
};

mod depth;
pub use depth::*;

mod entry;
pub use entry::*;

#[derive(Debug, Error, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum WithEntryError {
    #[error("expected an intermediate page, but found a terminating page")]
    TerminatingPage,

    #[error("page was not mapped")]
    NotMapped,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum CreateEntryError {
    #[error("expected an intermediate page, but found a terminating page")]
    TerminatingPage,

    #[error("ran out of memory for allocation")]
    OutOfMemory,
}

impl From<NextFrameError> for CreateEntryError {
    fn from(error: NextFrameError) -> Self {
        match error {
            NextFrameError::NoneFree => Self::OutOfMemory,
        }
    }
}

#[derive(Clone)]
pub(super) struct PageTable<BorrowKind: InteriorBorrow> {
    frame: Address<Frame>,
    depth: Depth,
    marker: PhantomData<BorrowKind>,
}

impl<BorrowKind: InteriorBorrow> PageTable<BorrowKind> {
    /// Creates a new [`PageTable`].
    ///
    /// # Safety
    ///
    /// - `frame` must either be an existing, well-constructed page table, or a
    ///   newly allocated and empty frame.
    /// - `depth` must be the correct paging depth associated with the page
    ///   table's frame.
    pub unsafe fn new(frame: Address<Frame>, depth: Depth) -> Self {
        Self {
            frame,
            depth,
            marker: PhantomData,
        }
    }

    fn table(&self) -> &[Entry; table_index_size()] {
        let page = HigherHalfDirectMap::frame_to_page(self.frame);

        debug_assert!(page.index() > 0);

        // Safety: All addresses in the higher-half direct map will be non-zero.
        let page_address = unsafe { NonZero::<usize>::new_unchecked(page.get().get()) };
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(page_address);
        let table_ptr = NonNull::slice_from_raw_parts(table_ptr, table_index_size());

        // Safety:
        // - Pointer came from an `Address<Page>`, so is naturally aligned to a page
        //   boundary.
        // - Pointer is from exposed provenance, and so is naturally dereferenceable (as
        //   it does not originate from an allocation).
        // - `Self::new` requires that the source frame be valid for readingh as a page
        //   table.
        // - Pointer is aliased as the same kind of borrow as `self`.
        let table = unsafe { table_ptr.as_ref() };

        table.try_into().unwrap()
    }

    pub fn sub_table(&self, index: usize) -> Option<PageTable<SharedBorrow>> {
        self.get_entry(index)
            .and_then(Entry::get_frame)
            .map(|frame| PageTable::<SharedBorrow> {
                frame,
                depth: self.depth.next(),
                marker: PhantomData,
            })
    }

    pub fn get_entry(&self, index: usize) -> Option<&Entry> {
        self.table().get(index)
    }

    pub fn with_entry<T>(
        &self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        let entry = self.get_entry(entry_index);
        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { entry.unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            // This is a simple runtime check to ensure we don't accidentally mistakenly
            // create large/huge pages along a table walk path.
            if !is_intermediate_entry(entry) {
                return Err(WithEntryError::TerminatingPage);
            }

            self.sub_table(entry_index)
                .ok_or(WithEntryError::NotMapped)
                .and_then(|page_table| page_table.with_entry(page, to_depth, with_fn))
        }
    }

    pub fn iter(&self) -> core::slice::Iter<Entry> {
        self.table().iter()
    }
}

impl PageTable<ExclusiveBorrow> {
    fn table_mut(&mut self) -> &mut [Entry; table_index_size()] {
        let page = HigherHalfDirectMap::frame_to_page(self.frame);

        debug_assert!(page.index() > 0);

        // Safety: All addresses in the higher-half direct map will be non-zero.
        let page_address = unsafe { NonZero::<usize>::new_unchecked(page.get().get()) };
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(page_address);
        let mut table_ptr = NonNull::slice_from_raw_parts(table_ptr, table_index_size());

        // Safety:
        // - Pointer came from an `Address<Page>`, so is naturally aligned to a page
        //   boundary.
        // - Pointer is from exposed provenance, and so is naturally dereferenceable (as
        //   it does not originate from an allocation).
        // - `Self::new` requires that the source frame be valid for readingh as a page
        //   table.
        // - Pointer is aliased as the same kind of borrow as `self`.
        let table = unsafe { table_ptr.as_mut() };

        table.try_into().unwrap()
    }

    pub fn sub_table_mut(&mut self, index: usize) -> Option<PageTable<ExclusiveBorrow>> {
        self.get_entry(index)
            .and_then(Entry::get_frame)
            .map(|frame| PageTable::<ExclusiveBorrow> {
                frame,
                depth: self.depth.next(),
                marker: PhantomData,
            })
    }

    pub fn get_entry_mut(&mut self, index: usize) -> Option<&mut Entry> {
        self.table_mut().get_mut(index)
    }

    pub fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        let entry = self.get_entry_mut(entry_index);
        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { entry.unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            // This is a simple runtime check to ensure we don't accidentally mistakenly
            // create large/huge pages along a table walk path.
            if !is_intermediate_entry(entry) {
                return Err(WithEntryError::TerminatingPage);
            }

            self.sub_table_mut(entry_index)
                .ok_or(WithEntryError::NotMapped)
                .and_then(|mut page_table| page_table.with_entry_mut(page, to_depth, with_fn))
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the
    /// given entry's frame, or creates the page table if it doesn't exist.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        to_depth: Depth,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, CreateEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        let entry = self.get_entry_mut(entry_index);
        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { entry.unwrap_unchecked() };

        if current_depth == to_depth {
            Ok(with_fn(entry))
        } else {
            // Ensure we don't mistakenly create large/huge pages along a table
            // walk path.
            if !is_intermediate_entry(entry) {
                return Err(CreateEntryError::TerminatingPage);
            }

            if !entry.is_enabled() {
                trace!(
                    "Creating: {{ page: {page:X?}, to_depth: {}, current_depth: {current_depth:?} }}",
                    to_depth.get(),
                );

                #[cfg(target_arch = "x86_64")]
                {
                    // x86 takes the most retrictive combination of all
                    // permissions from the intermediate and leaf entries,
                    // which means that a read-only intermediate page table
                    // entry will make the entire block of memory represented
                    // by the entry read-only, regardless of the leaf entry's
                    // permissions.
                    entry.set_write_execute();

                    // Insert the `USER` bit in all non-leaf, non-higher-half
                    // pages. This is for compatibility with the x86 paging
                    // scheme, where non-`USER` pages in a page table walk will
                    // immediately return an access error.
                    if !HigherHalfDirectMap::is_address_higher_half(page.get()) {
                        entry.set_user(true);
                    }
                }

                let frame = PhysicalMemoryManager::next_free(core::num::NonZero::<usize>::MIN,true)?;

                // Safety: Frame is unused.
                unsafe {
                    entry.set_frame(frame);
                }

                // Safety: Entry is being enabled.
                unsafe {
                    entry.set_enabled(true);
                }

                trace!("Created: {entry:X?}");
            }

            let page_table = self.sub_table_mut(entry_index);
            debug_assert!(page_table.is_some());

            // Safety: If page table didn't exist, it was just created.
            let mut page_table = unsafe { page_table.unwrap_unchecked() };
            page_table.with_entry_create(page, to_depth, with_fn)
        }
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<Entry> {
        self.table_mut().iter_mut()
    }
}

impl<BorrowKind: InteriorBorrow> core::fmt::Debug for PageTable<BorrowKind> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        writeln!(f, "PageTable {{")?;

        self.table()
            .iter()
            .enumerate()
            .try_for_each(|(index, entry)| writeln!(f, "    {index: >3}: {entry:X?}"))?;

        write!(f, "}}")?;

        Ok(())
    }
}

fn is_intermediate_entry(entry: &Entry) -> bool {
    cfg_select! {
        target_arch = "x86_64" => { !entry.is_huge() }

        _ => { todo!() }
    }
}
