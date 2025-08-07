use crate::mem::{
    HigherHalfDirectMap,
    pmm::{NextFrameError, PhysicalMemoryManager},
};
use libsys::{
    address::{Address, Page},
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

#[repr(C)]
#[cfg_attr(target_arch = "x86_64", repr(align(0x1000)))]
#[derive(Debug, Clone)]
pub(super) struct PageTable([Entry; table_index_size()]);

assert_eq_size!([u8; 0x1000], PageTable);

impl PageTable {
    pub const fn empty() -> Self {
        Self([const { Entry::empty() }; table_index_size()])
    }

    pub fn get(&self, index: usize) -> Option<&Entry> {
        self.0.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Entry> {
        self.0.get_mut(index)
    }

    /// # Safety
    ///
    /// - `current_depth` must be the current depth of the paging traversal.
    pub unsafe fn with_entry<T>(
        &self,
        page: Address<Page>,
        current_depth: Depth,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get(entry_index).unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            // This is a simple runtime check to ensure we don't accidentally mistakenly create
            // large/huge pages along a table walk path.
            if !is_intermediate_entry(entry) {
                return Err(WithEntryError::TerminatingPage);
            }

            entry
                .page_table()
                .ok_or(WithEntryError::NotMapped)
                .and_then(|page_table| {
                    // Safety: Caller is required to maintain safety invariants.
                    unsafe { page_table.with_entry(page, current_depth.next(), to_depth, with_fn) }
                })
        }
    }

    pub unsafe fn with_entry_mut<T>(
        &mut self,
        page: Address<Page>,
        current_depth: Depth,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get_mut(entry_index).unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            // This is a simple runtime check to ensure we don't accidentally mistakenly create
            // large/huge pages along a table walk path.
            if !is_intermediate_entry(entry) {
                return Err(WithEntryError::TerminatingPage);
            }

            entry
                .page_table_mut()
                .ok_or(WithEntryError::NotMapped)
                .and_then(|page_table| {
                    // Safety: Caller is required to maintain safety invariants.
                    unsafe {
                        page_table.with_entry_mut(page, current_depth.next(), to_depth, with_fn)
                    }
                })
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the given entry's frame,
    /// or creates the page table if it doesn't exist.
    pub fn with_entry_create<T>(
        &mut self,
        page: Address<Page>,
        current_depth: Depth,
        to_depth: Depth,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, CreateEntryError> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get_mut(entry_index).unwrap_unchecked() };

        if current_depth == to_depth {
            Ok(with_fn(entry))
        } else {
            // This is a simple runtime check to ensure we don't accidentally mistakenly create
            // large/huge pages along a table walk path.
            if !is_intermediate_entry(entry) {
                return Err(CreateEntryError::TerminatingPage);
            }

            if !entry.is_enabled() {
                trace!(
                    "Creating table: {{ page: {page:X?}, to_depth: {}, current_depth: {} }}",
                    to_depth.get(),
                    current_depth.get()
                );

                // Insert the `USER` bit in all non-leaf, non-higher-half pages. This is for
                // compatibility with the x86 paging scheme, where non-`USER` pages in a
                // page table walk will immediately return an access error.
                #[cfg(target_arch = "x86_64")]
                if !HigherHalfDirectMap::is_address_higher_half(page.get()) {
                    entry.set_user(true);
                }

                let frame = PhysicalMemoryManager::next_frame(true)?;

                // Safety: Frame is unused.
                unsafe {
                    entry.set_frame(frame);
                }

                entry.set_enabled(true);
            }

            let page_table = entry.page_table_mut();
            debug_assert!(page_table.is_some());

            // Safety: If page table didn't exist, it was just created.
            let page_table = unsafe { page_table.unwrap_unchecked() };

            page_table.with_entry_create(page, current_depth.next(), to_depth, with_fn)
        }
    }

    pub fn iter(&self) -> core::slice::Iter<Entry> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> core::slice::IterMut<Entry> {
        self.0.iter_mut()
    }
}

fn is_intermediate_entry(entry: &Entry) -> bool {
    cfg_select! {
        target_arch = "x86_64" => { !entry.is_huge() }

        _ => { todo!() }
    }
}
