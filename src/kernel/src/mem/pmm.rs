use crate::{interrupts::InterruptCell, mem::HigherHalfDirectMap};
use bitvec::slice::BitSlice;
use core::{num::NonZero, sync::atomic::AtomicUsize};
use libsys::{
    address::{Address, Frame},
    constants::{page_bits, page_mask, page_size},
    math::align_up_div,
};
use spin::RwLock;

#[derive(Debug, Error)]
pub enum NextFrameError {
    #[error("the physical memory manager is out of free frames")]
    NoneFree,
}

#[derive(Debug, Error)]
pub enum LockFrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,

    #[error("cannot lock frame; not free")]
    NotLocked,
}

#[derive(Debug, Error)]
pub enum FreeFrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,

    #[error("cannot free frame; not locked")]
    NotLocked,
}

crate::singleton! {
    pub PhysicalMemoryManager {
        table: InterruptCell<RwLock<&'static mut BitSlice<AtomicUsize>>>,
        total_frames: usize
    }

    /// Initializes the static physical memory manager with the provided bootloader memory map request.
    fn init(memory_map_request: &limine::request::MemoryMapRequest) -> Self {
        let memory_map = memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries();

        let last_entry = memory_map.last().unwrap();

        // While this is the ""total"" physical memory, it should be noted it isn't the total *installed* memory.
        // Because of hardware addressing, reserved regions—and other quirks—this number will likely be much larger
        // than the actual amount of installed physical memory the machine has.
        let total_physical_memory =
            usize::try_from(last_entry.base + last_entry.length).unwrap();

        let total_frames = align_up_div(total_physical_memory, page_bits());
        trace!("Total frames: {total_frames} ({total_physical_memory:#X} Bytes)");

        // Aligned frame count to the next multiple of `usize`s bit count.
        let table_slice_len = align_up_div(
            total_frames,
            NonZero::new(usize::BITS.trailing_zeros()).unwrap(),
        );
        // Total memory the table will consume as a multiple of frame size.
        let table_area_in_frames = align_up_div(
            table_slice_len * core::mem::size_of::<usize>(),
            page_bits(),
        );
        // Total memory the table will consume as a multiple of bytes.
        let table_area_in_bytes = table_area_in_frames * page_size();
        trace!(
            "Table Size: {table_slice_len:#X}, Table Area (Frames): {table_area_in_frames:#X}, Table Area (Bytes): {table_area_in_bytes:#X}"
        );

        // Select a region that will fit the table, aligned to frame size.
        let select_region = memory_map
            .iter()
            .filter(|entry| entry.entry_type == limine::memory_map::EntryType::USABLE)
            .map(|entry| {
                let entry_start = usize::try_from(entry.base).unwrap();
                let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                entry_start..entry_end
            })
            .find(|region| region.len() >= table_area_in_bytes)
            .map(|region| region.start..(region.start + table_area_in_bytes))
            .expect("no memory regions large enough for frame table");

        debug_assert_eq!(select_region.start & page_mask(), 0);
        debug_assert_eq!(select_region.end & page_mask(), 0);

        trace!("Frame table region: {select_region:#X?}");

        let table_ptr = core::ptr::with_exposed_provenance_mut::<u8>(
            HigherHalfDirectMap::offset(select_region.start).get(),
        );

        // Pre-initialize the table memory to a known, zeroed out state.
        // Safety: The memory region should not be in use by any other context.
        unsafe {
            core::ptr::write_bytes(table_ptr, 0, table_area_in_bytes);
        }

        let table = BitSlice::from_slice_mut({
            // Safety: Region is guaranteed by the memory map to be unused, and has been zero-initialized to be valid as `AtomicUsize`.
            #[allow(clippy::cast_ptr_alignment)]
            unsafe {
                core::slice::from_raw_parts_mut(
                    table_ptr.cast::<AtomicUsize>(),
                    table_slice_len,
                )
            }
        });

        // Fill the padding bits, as the table may have more bits than there are frames.
        table
            .get_mut(total_frames..)
            .expect("attempted to index frame table out of bounds")
            .fill(true);

        // Ensure the table's frames are reserved.
        trace!("Locking (Table): {select_region:#X?}");
        table
            .get_mut((select_region.start / page_size())..(select_region.end / page_size()))
            .expect("attempted to index frame table out of bounds")
            .fill(true);

        let mut prev_entry_range_end = None;
        memory_map
            .iter()
            .map(|entry| {
                // Map the entry to a usable range and type

                let entry_start = usize::try_from(entry.base).unwrap();
                let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                (entry_start..entry_end, entry.entry_type)
            })
            .for_each(|(entry_range, entry_ty)| {
                // If there's space inbetween entries, we'll lock it to ensure it isn't accidentally used.
                if let Some(prev_entry_range_end) = prev_entry_range_end
                    && prev_entry_range_end < entry_range.start
                {
                    trace!(
                        "Locking (Inbetween): {:#X?}",
                        prev_entry_range_end..entry_range.start
                    );

                    table
                        .get_mut(
                            (prev_entry_range_end / page_size())
                                ..(entry_range.start / page_size()),
                        )
                        .expect("attempted to index frame table out of bounds")
                        .fill(true);
                }

                // Only lock the non-usable entries...
                if entry_ty != limine::memory_map::EntryType::USABLE {
                    trace!("Locking (Used): {entry_range:#X?}");

                    table
                        .get_mut(
                            (entry_range.start / page_size())..(entry_range.end / page_size()),
                        )
                        .expect("attempted to index frame table out of bounds")
                        .fill(true);
                }

                prev_entry_range_end = Some(entry_range.end);
            });

        Self {
            table: InterruptCell::new(spin::RwLock::new(table)),
            total_frames,
        }
    }
}

// Safety: Type uses entirely atomic operations.
unsafe impl Send for PhysicalMemoryManager {}
// Safety: Type uses entirely atomic operations.
unsafe impl Sync for PhysicalMemoryManager {}

impl PhysicalMemoryManager {
    /// Passes the static physical memory manager's frame table to `with_fn`,
    /// returning the result.
    fn with_table<T>(with_fn: impl FnOnce(&RwLock<&'static mut BitSlice<AtomicUsize>>) -> T) -> T {
        Self::get_static().table.with(with_fn)
    }

    pub fn total_frames() -> usize {
        Self::get_static().total_frames
    }

    pub fn total_memory() -> usize {
        Self::total_frames() * page_size()
    }

    pub fn next_frame(clear_memory: bool) -> Result<Address<Frame>, NextFrameError> {
        Self::with_table(|table| {
            let mut table = table.write();
            let free_frame_index = table.first_zero().ok_or(NextFrameError::NoneFree)?;

            // Safety: `index` is returned from a search function on `Self`.
            unsafe {
                table.set_unchecked(free_frame_index, true);
            }

            trace!(
                "Frame Locked: {:#X?}",
                free_frame_index << page_bits().get()
            );

            let frame = Address::<Frame>::new(free_frame_index << page_bits().get())
                .expect("physical memory manager constructed invalid physical address");

            if clear_memory {
                // Safety: Memory was just allocated, and is not currently aliased.
                unsafe {
                    crate::mem::zero_frame(frame);
                }
            }

            Ok(frame)
        })
    }

    pub fn next_frames(
        count: NonZero<usize>,
        align_bits: Option<NonZero<u32>>,
        clear_memory: bool,
    ) -> Result<Address<Frame>, NextFrameError> {
        Self::with_table(|table| {
            let mut table = table.write();

            let align_bits = align_bits.unwrap_or(NonZero::<u32>::MIN).get();
            let align_index_skip = u32::max(1, align_bits >> page_bits().get());

            let free_frames_index = table
                .windows(count.get())
                .enumerate()
                .step_by(align_index_skip.try_into().unwrap())
                .find_map(|(index, window)| window.not_any().then_some(index))
                .ok_or(NextFrameError::NoneFree)?;

            // It's a bit uglier to find the index of the window, then effectively reacreate
            // it. However, `.windows()` does not return a mutable bitslice, so
            // this is how it must be done.
            let free_frames = table
                .get_mut(free_frames_index..(free_frames_index + count.get()))
                .unwrap();
            free_frames.fill(true);

            trace!(
                "Frames Locked: {:#X?}..{:#X?}",
                free_frames_index,
                free_frames_index + free_frames.len()
            );

            let frame = Address::<Frame>::new(free_frames_index << page_bits().get())
                .expect("physical memory manager constructed invalid physical address");

            if clear_memory {
                // Safety: Memory was just allocated, and is not currently aliased.
                unsafe {
                    crate::mem::zero_frame(frame);
                }
            }

            Ok(frame)
        })
    }

    pub fn lock_frame(address: Address<Frame>) -> Result<(), LockFrameError> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index >= Self::total_frames() {
                return Err(LockFrameError::OutOfBounds);
            }

            // Safety: Index is checked within bounds.
            if unsafe { *table.get_unchecked(index) } {
                return Err(LockFrameError::NotLocked);
            }

            // Safety: Index is checked within bounds.
            unsafe {
                table.set_aliased_unchecked(index, true);
            }

            trace!("Frame Locked: {:#X?}", index << page_bits().get());

            Ok(())
        })
    }

    pub fn free_frame(address: Address<Frame>) -> Result<(), FreeFrameError> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index >= Self::total_frames() {
                return Err(FreeFrameError::OutOfBounds);
            }

            // Safety: Index is checked within bounds.
            if !(unsafe { *table.get_unchecked(index) }) {
                return Err(FreeFrameError::NotLocked);
            }

            // Safety: Index is checked within bounds.
            unsafe {
                table.set_aliased_unchecked(index, false);
            }

            trace!("Freed: {:#X?}", index << page_bits().get());

            Ok(())
        })
    }

    pub fn is_locked(address: Address<Frame>) -> bool {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            assert!(index < Self::total_frames(), "frame index out of bounds");

            // Safety: Index is checked to be within frame bounds.
            unsafe { *table.get_unchecked(index) }
        })
    }
}
