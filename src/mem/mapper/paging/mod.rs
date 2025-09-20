use crate::{
    mem::{
        HigherHalfDirectMap,
        addr::{
            phys::{FrameAddress, HugeFrame, LargeFrame, PhysicalAddress, StandardFrame},
            virt::{StandardPage, VirtualAddress},
        },
        clear_frame_memory,
        pmm::PhysicalMemoryManager,
    },
    util::{ExclusiveBorrow, InteriorBorrow, SharedBorrow},
};
use core::{marker::PhantomData, num::NonZero, ptr::NonNull};

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
pub(super) struct PageTable<BorrowKind> {
    frame: StandardFrame,
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
    pub unsafe fn new(frame: StandardFrame, depth: Depth) -> Self {
        Self {
            frame,
            depth,
            marker: PhantomData,
        }
    }

    fn table(&self) -> &[Entry; PagingInfo::MAX_TABLE_INDEX.get()] {
        let table_address = HigherHalfDirectMap::frame_to_page::<_, StandardPage>(self.frame);
        let table_address = NonZero::<usize>::new(usize::from(table_address)).unwrap();
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(table_address);
        let table_ptr = NonNull::slice_from_raw_parts(table_ptr, PagingInfo::MAX_TABLE_INDEX.get());

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
            let frame = entry.get_address()?;
            let frame = StandardFrame::try_from(frame).unwrap();
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
        address: VirtualAddress,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(address);

        debug_assert!(entry_index < PagingInfo::MAX_TABLE_INDEX.get());

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
                .and_then(|page_table| page_table.with_entry(address, to_depth, with_fn))
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
    fn table_mut(&mut self) -> &mut [Entry; PagingInfo::MAX_TABLE_INDEX.get()] {
        let table_address = HigherHalfDirectMap::frame_to_page::<_, StandardPage>(self.frame);
        let table_address = NonZero::<usize>::new(usize::from(table_address)).unwrap();
        let table_ptr = NonNull::<Entry>::with_exposed_provenance(table_address);
        let mut table_ptr =
            NonNull::slice_from_raw_parts(table_ptr, PagingInfo::MAX_TABLE_INDEX.get());

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
            let frame = entry.get_address()?;
            let frame = StandardFrame::try_from(frame).unwrap();
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
        address: VirtualAddress,
        to_depth: Option<Depth>,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, WithEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(address);

        debug_assert!(entry_index < PagingInfo::MAX_TABLE_INDEX.get());

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
                .and_then(|mut page_table| page_table.with_entry_mut(address, to_depth, with_fn))
        }
    }

    /// Attempts to get a mutable reference to the page table that lies in the
    /// given entry's frame, or creates the page table if it doesn't exist.
    pub fn with_entry_create<T>(
        &mut self,
        address: VirtualAddress,
        to_depth: Depth,
        with_fn: impl FnOnce(&mut Entry) -> T,
    ) -> Result<T, CreateEntryError> {
        let current_depth = self.depth;
        let entry_index = current_depth.index_of(address);

        debug_assert!(entry_index < PagingInfo::MAX_TABLE_INDEX.get());

        // Safety: `entry_index`s maximum value is the same as the entry table size.
        let entry = unsafe { self.get_entry_mut(entry_index).unwrap_unchecked() };

        if current_depth == to_depth {
            trace!(
                "Modifying {{ Index: {entry_index}, Depth: {:?}/{:?}, {entry:X?} }}",
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
                    "Creating: {{ Index: {entry_index}, Depth: {:?}/{:?} }}",
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

                    if !HigherHalfDirectMap::is_address_higher_half(address) {
                        entry.set_user(true);
                    }
                }

                let frame = PhysicalMemoryManager::next_free_frame::<StandardFrame>()
                    .inspect(|frame| {
                        // Safety: Memory was just allocated, and is not otherwise aliased.
                        unsafe {
                            clear_frame_memory(*frame);
                        }
                    })
                    .ok_or(CreateEntryError::OutOfMemory)?;

                // Safety: Frame is unused.
                unsafe {
                    entry.set_address(frame);
                }

                entry.set_enabled();

                trace!("Created: {entry:X?}");
            }

            trace!(
                "Traversing: {{ Index: {entry_index}, Depth: {:?}/{:?}, {entry:X?} }}",
                current_depth.get(),
                to_depth.get()
            );

            // Safety: If page table didn't exist, it was just created.
            let mut page_table = unsafe { self.sub_table_mut(entry_index).unwrap_unchecked() };

            page_table.with_entry_create(address, to_depth, with_fn)
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

pub struct PagingInfo;

impl PagingInfo {
    pub const TABLE_INDEX_BITS: NonZero<u32> = NonZero::new(9).unwrap();
    pub const MAX_TABLE_INDEX: NonZero<usize> =
        NonZero::new(1usize << Self::TABLE_INDEX_BITS.get()).unwrap();
    pub const TABLE_INDEX_MASK: NonZero<usize> =
        NonZero::new(Self::MAX_TABLE_INDEX.get() - 1).unwrap();

    /// Whether the current environment supports 2MiB pages.
    pub fn is_large_pages_enabled() -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::{cpuid::feature_info, registers::control::cr4};

                debug_assert!(
                    feature_info().is_some_and(|cpuid| cpuid.has_pae())
                        && cr4::CR4::read().contains(cr4::Flags::PAE)
                );

                true
            }

            _ => { unimplemented!() }
        }
    }

    /// Whether the current environment supports 1GiB pages.
    pub fn is_huge_pages_enabled() -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                crate::arch::x86_64::cpuid::extended_feature_identifiers()
                    .is_some_and(|cpuid| cpuid.has_1gib_pages())
            }

            _ => { unimplemented!() }
        }
    }
}

impl core::fmt::Debug for PagingInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PagingInfo")
            .field("Large Pages Enabled", &Self::is_large_pages_enabled())
            .field("Large Page Size", &LargeFrame::SIZE_IN_BYTES.get())
            .field("Huge Pages Enabled", &Self::is_huge_pages_enabled())
            .field("Huge Page Size", &HugeFrame::SIZE_IN_BYTES.get())
            .field(
                "Physical Address Bits",
                &PhysicalAddress::canonical_bits().get(),
            )
            .field(
                "Virtual Address Bits",
                &VirtualAddress::canonical_bits().get(),
            )
            .field("Page Table Index Bits", &Self::TABLE_INDEX_BITS.get())
            .field("Page Table Index Mask", &Self::TABLE_INDEX_MASK.get())
            .field("Page Table Max Index", &Self::MAX_TABLE_INDEX.get())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use crate::mem::mapper::paging::PagingInfo;
    use core::num::NonZero;

    fn page_table_info_index_bits() {
        assert_eq!(
            PagingInfo::TABLE_INDEX_BITS,
            NonZero::<u32>::new(9).unwrap()
        );
    }

    fn page_table_info_max_index() {
        assert_eq!(
            PagingInfo::MAX_TABLE_INDEX,
            NonZero::<usize>::new(512).unwrap()
        );
    }

    fn page_table_info_non_index_bit_mask() {
        assert_eq!(
            PagingInfo::TABLE_INDEX_MASK,
            NonZero::<usize>::new(0x1FF).unwrap()
        );
    }
}
