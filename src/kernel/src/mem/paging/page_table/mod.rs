use crate::mem::{HigherHalfDirectMap, paging::Error};
use libsys::{Address, Page, table_index_size};

mod depth;
pub use depth::*;

mod entry;
pub use entry::*;

#[repr(C)]
#[cfg_attr(target_arch = "x86_64", repr(align(0x1000)))]
#[derive(Debug, FromZeros, Clone, Copy)]
pub(super) struct PageTable([Entry; table_index_size()]);

impl PageTable {
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
    ) -> Result<T, Error> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get(entry_index).unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            if !entry.is_intermediate() {
                return Err(Error::TerminatingPage);
            }

            entry
                .page_table()
                .ok_or(Error::NotMapped(page))
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
    ) -> Result<T, Error> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get_mut(entry_index).unwrap_unchecked() };

        if current_depth == to_depth.unwrap_or(Depth::max()) {
            Ok(with_fn(entry))
        } else {
            if !entry.is_intermediate() {
                return Err(Error::TerminatingPage);
            }

            entry
                .page_table_mut()
                .ok_or(Error::NotMapped(page))
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
    ) -> Result<T, Error> {
        let entry_index = current_depth.index_of(page.get());

        debug_assert!(entry_index < table_index_size());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get_mut(entry_index).unwrap_unchecked() };

        if current_depth == to_depth {
            Ok(with_fn(entry))
        } else {
            if !entry.is_intermediate() {
                return Err(Error::TerminatingPage);
            }

            if !entry.is_enabled() {
                debug_assert!(
                    entry.get_frame() == Address::default(),
                    "entry is disabled, but has an address: {current_depth:?} {entry:?}"
                );

                trace!(
                    "Creating table: {page:X?} (to_depth:{}, current:{})",
                    to_depth.get(),
                    current_depth.get()
                );

                entry
                    .populate()
                    .expect("could not allocate memory for new page table");

                // Architecture-specific bit enables.
                cfg_select! {
                    target_arch = "x86_64" => {
                        // Insert the `USER` bit in all non-leaf, non-higher-half pages. This is for
                        // compatibility with the x86 paging scheme, where non-`USER` pages in a page table
                        // walk will immediately return an access error.
                        if !HigherHalfDirectMap::is_address_higher_half(page.get()) {
                            unsafe {
                                entry.set_user_accessible(true);
                            }
                        }
                    }

                    target_arch = "riscv64" => {
                        // RISC-V supports a "global" bit to indicate a translation step is
                        // identical between all address spaces. In our case, this is for kernel
                        // pages, which will be mapped into the higher-half in all address spaces.
                        if HigherHalfDirectMap::is_address_higher_half(page.get()) {
                            unsafe {
                                entry.set_global(true);
                            }
                        }
                    }
                }

                unsafe {
                    entry.set_permissions(crate::mem::Permissions::ReadOnly);
                }

                unsafe {
                    entry.set_enabled(true);
                }
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
