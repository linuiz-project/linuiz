use crate::{
    mem::{
        HigherHalfDirectMap,
        pmm::{PageSize, PhysicalMemoryManager},
    },
    util::{ExclusiveBorrow, InteriorBorrow, SharedBorrow},
};
use core::{marker::PhantomData, ptr::NonNull};
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
        let table_address = HigherHalfDirectMap::offset(self.frame.get().get());
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(table_address);
        let table_ptr = NonNull::slice_from_raw_parts(table_ptr, table_index_size());

        // Safety:
        // - Pointer is from `Address<Frame>`, so is naturally page-aligned.
        // - Pointer is from exposed provenance, and so is naturally dereferenceable (as
        //   it does not originate from an allocation).
        // - `Self::new` requires that the `self.frame` be valid as a page table.
        // - Pointer is aliased identically to `self`.
        // - `Self::new` required that `self.frame` be at least zero initialized.
        let table = unsafe { table_ptr.as_ref() };

        table.try_into().unwrap()
    }

    pub fn sub_table(&self, index: usize) -> Option<PageTable<SharedBorrow>> {
        self.get_entry(index).and_then(|entry| {
            let frame = entry.get_frame()?;
            let next_depth = self.depth.next_checked()?;

            Some(PageTable::<SharedBorrow> {
                frame,
                depth: next_depth,
                marker: PhantomData,
            })
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
            if !entry.is_intermediate() {
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

    pub fn walk_all(&self, func: impl Fn(Depth, usize, &Entry) + Copy) {
        let current_depth = self.depth;
        self.table()
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry.is_enabled())
            .for_each(|(index, entry)| {
                func(current_depth, index, entry);

                if let Some(next_page_table) = self.sub_table(index) {
                    next_page_table.walk_all(func);
                }
            });
    }
}

impl PageTable<ExclusiveBorrow> {
    fn table_mut(&mut self) -> &mut [Entry; table_index_size()] {
        let table_address = HigherHalfDirectMap::offset(self.frame.get().get());
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(table_address);
        let mut table_ptr = NonNull::slice_from_raw_parts(table_ptr, table_index_size());

        // Safety:
        // - Pointer is from `Address<Frame>`, so is naturally page-aligned.
        // - Pointer is from exposed provenance, and so is naturally dereferenceable (as
        //   it does not originate from an allocation).
        // - `Self::new` requires that the `self.frame` be valid as a page table.
        // - Pointer is aliased identically to `self`.
        // - `Self::new` required that `self.frame` be at least zero initialized.
        let table = unsafe { table_ptr.as_mut() };

        table.try_into().unwrap()
    }

    pub fn sub_table_mut(&mut self, index: usize) -> Option<PageTable<ExclusiveBorrow>> {
        self.get_entry(index).and_then(|entry| {
            let frame = entry.get_frame()?;
            let next_depth = self.depth.next_checked()?;

            Some(PageTable::<ExclusiveBorrow> {
                frame,
                depth: next_depth,
                marker: PhantomData,
            })
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
            if !entry.is_intermediate() {
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
            trace!(
                "Modifying: {:X?} {{ Index: {entry_index}, Depth: {:?}/{:?} }}",
                page.get().get(),
                current_depth.get(),
                to_depth.get()
            );

            Ok(with_fn(entry))
        } else {
            // Ensure we don't create oversized pages along a table walk path.
            if !entry.is_intermediate() {
                return Err(CreateEntryError::TerminatingPage);
            }

            if !entry.is_enabled() {
                // We'll populate the entry in this case, to ensure we can continue traversing.

                trace!(
                    "Creating: {:X?} {{ Index: {entry_index}, Depth: {:?}/{:?} }}",
                    page.get().get(),
                    current_depth.get(),
                    to_depth.get()
                );

                #[cfg(target_arch = "x86_64")]
                {
                    // x86 takes the most retrictive combination of all
                    // permissions from the intermediate and leaf entries,
                    // which means that a read-only intermediate page table
                    // entry will make the entire block of memory represented
                    // by the entry read-only, regardless of the leaf entry's
                    // permissions.
                    //
                    // This mean every intermediate entry needs to be `WRITE`+`EXECUTE`, and any
                    // intermediate entries that will lead to userspace also need to be marked
                    // `USER`.

                    entry.set_write_execute();

                    if !HigherHalfDirectMap::is_address_higher_half(page.get()) {
                        entry.set_user(true);
                    }
                }

                let frame = PhysicalMemoryManager::next_free_frame(PageSize::Standard, true)
                    .ok_or(CreateEntryError::OutOfMemory)?;

                // Safety: Frame is unused.
                unsafe {
                    entry.set_frame(frame);
                }

                entry.set_enabled();

                trace!("Created: {entry:X?}");
            }

            let page_table = self.sub_table_mut(entry_index);
            debug_assert!(page_table.is_some());
            // Safety: If page table didn't exist, it was just created.
            let mut page_table = unsafe { page_table.unwrap_unchecked() };
            trace!(
                "Traversing: {:X?} {{ Index: {entry_index}, Depth: {:?}/{:?} }}",
                page.get().get(),
                current_depth.get(),
                to_depth.get()
            );

            page_table.with_entry_create(page, to_depth, with_fn)
        }
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<Entry> {
        self.table_mut().iter_mut()
    }
}

impl<BorrowKind: InteriorBorrow> core::fmt::Debug for PageTable<BorrowKind> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_map().entries(self.iter().enumerate()).finish()
    }
}
