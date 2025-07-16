use crate::{interrupts::InterruptCell, mem::HigherHalfDirectMap};
use bitvec::slice::BitSlice;
use core::{num::NonZero, sync::atomic::AtomicUsize};
use libsys::{Address, Frame, align_up_div, page_mask, page_shift, page_size};
use spin::RwLock;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    #[error("the physical memory manager is out of free frames")]
    NoneFree,

    #[error("given alignment is invalid (e.g. not a power-of-two)")]
    InvalidAlignment,

    #[error("attempted to index out of bounds: {0:#X?}")]
    OutOfBounds(Address<Frame>),

    #[error("cannot lock; frame not free: {0:#X?}")]
    NotFree(Address<Frame>),

    #[error("cannot free; frame not locked: {0:#X?}")]
    NotLocked(Address<Frame>),
}

type FrameTable = RwLock<&'static mut BitSlice<AtomicUsize>>;

crate::singleton! {
    pub PhysicalMemoryManager {
        table: InterruptCell<FrameTable>,
        total_frames: usize,
    }

    /// Initializes the static physical memory manager with the provided bootloader memory map request.
    fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        let memory_map = memory_map_request
            .get_response()
            .expect("no response to memory map request")
            .entries();

        report_memory_map_entries(memory_map);
        report_total_usable_memory(memory_map);

        let last_entry = memory_map.last().unwrap();

        // While this is the ""total"" physical memory, it should be noted it isn't the total *installed* memory.
        // Because of hardware addressing, reserved regions—and other quirks—this number will likely be much larger
        // than the actual amount of installed physical memory the machine has.
        let total_physical_memory =
            usize::try_from(last_entry.base + last_entry.length).unwrap();

        let total_frames = align_up_div(total_physical_memory, page_shift());
        trace!("Total frames: {total_frames} ({total_physical_memory:#X} B)");

        // Aligned frame count to the next multiple of `usize`s bit count.
        let table_slice_len = align_up_div(
            total_frames,
            NonZero::new(usize::BITS.trailing_zeros()).unwrap(),
        );
        // Total memory the table will consume as a multiple of frame size.
        let table_area_in_frames = align_up_div(
            table_slice_len * core::mem::size_of::<usize>(),
            page_shift(),
        );
        // Total memory the table will consume as a multiple of bytes.
        let table_area_in_bytes = table_area_in_frames * page_size();
        trace!(
            "Table Size: {table_slice_len:#X}, Table Area (Frames): {table_area_in_frames:#X}, Table Area (Bytes): {table_area_in_bytes:#X}"
        );

        // Select a region that will fit the table, aligned to frame size.
        // TODO allow selecting a region that would fit the table, but whose beginning does not align to a frame boundary.
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
        trace!(
            "Locking (Table): {:#X}..{:#X}",
            select_region.start, select_region.end
        );
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
                    debug!(
                        "Locking (Inbetween): {:#X}..{:#X}",
                        prev_entry_range_end, entry_range.start
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
                    trace!("Locking: {:#X}..{:#X}", entry_range.start, entry_range.end);

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
    /// Passes the static physical memory manager's frame table to `with_fn`, returning the result.
    fn with_table<T>(with_fn: impl FnOnce(&FrameTable) -> Result<T, Error>) -> Result<T, Error> {
        Self::get_static().table.with(with_fn)
    }

    pub fn total_frames() -> usize {
        Self::get_static().total_frames
    }

    pub fn total_memory() -> usize {
        Self::total_frames() * libsys::page_size()
    }

    pub fn next_frame() -> Result<Address<Frame>, Error> {
        Self::with_table(|table| {
            let mut table = table.write();
            let index = table.first_zero().ok_or(Error::NoneFree)?;

            // Safety: `index` is returned from a search function on `Self`.
            unsafe {
                table.set_unchecked(index, true);
            }

            trace!("Frame Locked: {:#X?}", index << page_shift().get());

            Ok(Address::new(index << page_shift().get()).unwrap())
        })
    }

    pub fn next_frames(
        count: NonZero<usize>,
        align_bits: Option<NonZero<u32>>,
    ) -> Result<Address<Frame>, Error> {
        Self::with_table(|table| {
            let mut table = table.write();

            let align_bits = align_bits.unwrap_or(NonZero::<u32>::MIN).get();
            let align_index_skip = u32::max(1, align_bits >> page_shift().get());

            let free_frames_index = table
                .windows(count.get())
                .enumerate()
                .step_by(align_index_skip.try_into().unwrap())
                .find_map(|(index, window)| window.not_any().then_some(index))
                .ok_or(Error::NoneFree)?;

            // It's a bit uglier to find the index of the window, then effectively reacreate it. However, `.windows()`
            // does not return a mutable bitslice, so this is how it must be done.
            let free_frames = table
                .get_mut(free_frames_index..(free_frames_index + count.get()))
                .unwrap();
            free_frames.fill(true);

            trace!(
                "Frames Locked: {:#X?}..{:#X?}",
                free_frames_index,
                free_frames_index + free_frames.len()
            );

            Ok(Address::new(free_frames_index << page_shift().get()).unwrap())
        })
    }

    pub fn lock_frame(address: Address<Frame>) -> Result<(), Error> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index < Self::total_frames() {
                // Make sure frame is free (bit is false) before we try to lock ...
                if !table[index] {
                    // Safety: Index is checked to be within frame bounds.
                    unsafe {
                        table.set_aliased_unchecked(index, true);
                    }

                    trace!("Frame Locked: {:#X?}", index << page_shift().get());

                    Ok(())
                } else {
                    Err(Error::NotFree(address))
                }
            } else {
                Err(Error::OutOfBounds(address))
            }
        })
    }

    pub fn free_frame(address: Address<Frame>) -> Result<(), Error> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index < Self::total_frames() {
                // Make sure frame is locked (bit is true) before we try to free ...
                if table[index] {
                    // Safety: Index is checked to be within frame bounds.
                    unsafe {
                        table.set_aliased_unchecked(index, false);
                    }

                    trace!("Freed: {:#X?}", index << page_shift().get());

                    Ok(())
                } else {
                    Err(Error::NotLocked(address))
                }
            } else {
                Err(Error::OutOfBounds(address))
            }
        })
    }

    pub fn is_locked(address: Address<Frame>) -> Result<bool, Error> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            if index < Self::total_frames() {
                // Safety: Index is checked to be within frame bounds.
                Ok(unsafe { *table.get_unchecked(index) })
            } else {
                Err(Error::OutOfBounds(address))
            }
        })
    }
}

fn report_memory_map_entries(memory_map: &[&limine::memory_map::Entry]) {
    memory_map.iter().for_each(|entry| {
        let entry_start = entry.base;
        let entry_end = entry_start + entry.length;
        debug!(
            "Memory map entry: {:#X?}  {}",
            entry_start..entry_end,
            match entry.entry_type {
                limine::memory_map::EntryType::USABLE => "USABLE",
                limine::memory_map::EntryType::RESERVED => "RESERVED",
                limine::memory_map::EntryType::EXECUTABLE_AND_MODULES => "EXECUTABLE_AND_MODULES",
                limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE => "BOOTLOADER_RECLAIMABLE",
                limine::memory_map::EntryType::ACPI_RECLAIMABLE => "ACPI_RECLAIMABLE",
                limine::memory_map::EntryType::ACPI_NVS => "ACPI_NVS",
                limine::memory_map::EntryType::FRAMEBUFFER => "FRAMEBUFFER",
                limine::memory_map::EntryType::BAD_MEMORY => "BAD_MEMORY",

                _ => unreachable!("!! UNKOWN !!"),
            }
        );
    });
}

fn report_total_usable_memory(memory_map: &[&limine::memory_map::Entry]) {
    let total_usable_memory =
        memory_map
            .iter()
            .fold(0u64, |usable_memory_count, entry| match entry.entry_type {
                limine::memory_map::EntryType::USABLE
                | limine::memory_map::EntryType::EXECUTABLE_AND_MODULES
                | limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
                | limine::memory_map::EntryType::ACPI_RECLAIMABLE => {
                    usable_memory_count + entry.length
                }

                limine::memory_map::EntryType::RESERVED
                | limine::memory_map::EntryType::ACPI_NVS
                | limine::memory_map::EntryType::FRAMEBUFFER
                | limine::memory_map::EntryType::BAD_MEMORY => usable_memory_count,

                _ => unreachable!("unknown memory map entry type"),
            });

    debug!(
        "Detected system memory: {}MB",
        total_usable_memory / 1_000_000
    );
}
