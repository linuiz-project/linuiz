use crate::{
    mem::HigherHalfDirectMap,
    util::sync::{Once, RwLock},
};
use bitmap::{BitMap, BitMapError};
use core::num::NonZero;
use libsys::{
    address::{Address, Frame},
    constants::{page_bits, page_mask, page_size},
    math::align_up_div,
};

#[derive(Debug, Error)]
pub enum NextFrameError {
    #[error("the physical memory manager is out of free frames")]
    NoneFree,
}

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,
}

impl From<BitMapError> for FrameError {
    fn from(error: BitMapError) -> Self {
        match error {
            BitMapError::OutOfBounds => Self::OutOfBounds,
        }
    }
}

pub struct PhysicalMemoryManager<'a> {
    bitmap: RwLock<BitMap<'a>>,
    total_frames: usize,
}

static PHYSICAL_MEMORY_MANAGER: Once<PhysicalMemoryManager<'static>> = Once::new();

unsafe impl Send for PhysicalMemoryManager<'_> {}
unsafe impl Sync for PhysicalMemoryManager<'_> {}

impl<'a: 'static> PhysicalMemoryManager<'a> {
    /// Initializes the static physical memory manager with the provided
    /// bootloader memory map request.
    pub fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        PHYSICAL_MEMORY_MANAGER.call_once(|| {
            let memory_map = memory_map_request
                .get_response()
                .expect("bootloader did not provide a response to the memory map request")
                .entries();

            let last_entry = memory_map.last().unwrap();
            // While this is the ""total"" physical memory, it should be noted it isn't the
            // total *installed* memory. Because of hardware addressing, reserved
            // regions—and other quirks—this number will likely be much larger
            // than the actual amount of installed physical memory the machine has.
            let total_physical_memory =
                usize::try_from(last_entry.base + last_entry.length).unwrap();

            let total_frames = align_up_div(total_physical_memory, page_bits());
            trace!("Total frames: {total_frames} ({total_physical_memory:#X} Bytes)");

            // Aligned frame count to the next multiple of `usize`s bit count.
            let bitmap_size = align_up_div(
                total_frames,
                NonZero::new(usize::BITS.trailing_zeros()).unwrap(),
            );
            // Total memory the bitmap will consume as a multiple of frame size.
            let bitmap_size_in_frames =
                align_up_div(bitmap_size * core::mem::size_of::<usize>(), page_bits());
            // Total memory the bitmap will consume as a multiple of bytes.
            let bitmap_size_in_bytes = bitmap_size_in_frames * page_size();

            // Inlining the format args breaks rustfmt for some reason.
            #[allow(clippy::uninlined_format_args)]
            {
                trace!(
                    "Bitmap {{ Size: {:#X}, Area (Frames): {:#X}, Area (Bytes): {:#X} }}",
                    bitmap_size, bitmap_size_in_frames, bitmap_size_in_bytes
                );
            }

            // Select a region that will fit the bitmap, aligned to frame size.
            let bitmap_region = memory_map
                .iter()
                .filter(|entry| entry.entry_type == limine::memory_map::EntryType::USABLE)
                .map(|entry| {
                    let entry_start = usize::try_from(entry.base).unwrap();
                    let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                    entry_start..entry_end
                })
                .find(|region| region.len() >= bitmap_size_in_bytes)
                .map(|region| region.start..(region.start + bitmap_size_in_bytes))
                .expect("no memory regions large enough for frame bitmap");

            debug_assert_eq!(bitmap_region.start & page_mask(), 0);
            debug_assert_eq!(bitmap_region.end & page_mask(), 0);

            trace!("Frame bitmap region: {bitmap_region:#X?}");

            // Construct the ptr based on an offset into the higher-half direct map.
            let bitmap_ptr = core::ptr::with_exposed_provenance_mut::<usize>(
                HigherHalfDirectMap::offset(bitmap_region.start).get(),
            );

            // Pre-initialize the bitmap memory to a known, zeroed out state.
            // Safety: The memory region should not be in use by any other context.
            unsafe {
                core::ptr::write_bytes(bitmap_ptr, 0, bitmap_size_in_bytes);
            }

            // Safety:
            // - Region is guaranteed by the memory map to be unused
            // - Region has been zero-initialized.
            let bitmap =
                unsafe { core::slice::from_raw_parts_mut::<'static>(bitmap_ptr, bitmap_size) };

            let mut bitmap = BitMap::<'static>::new(bitmap, total_frames);

            // Ensure the bitmap's frames are reserved.
            trace!("Locking: {bitmap_region:#X?}");
            let bitmap_region_start_index = bitmap_region.start / page_size();
            let bitmap_region_end_index = bitmap_region.end / page_size();
            bitmap
                .set(bitmap_region_start_index..bitmap_region_end_index)
                .unwrap();

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
                    // If there's space inbetween entries, we'll lock it to ensure it isn't
                    // accidentally used.
                    if let Some(prev_entry_range_end) = prev_entry_range_end
                        && prev_entry_range_end < entry_range.start
                    {
                        trace!(
                            "Locking (Inbetween): {:#X?}",
                            prev_entry_range_end..entry_range.start
                        );
                        let lock_start_index = prev_entry_range_end / page_size();
                        let lock_end_index = entry_range.start / page_size();
                        bitmap.set(lock_start_index..lock_end_index).unwrap();
                    }

                    // Only lock the non-usable entries...
                    if entry_ty != limine::memory_map::EntryType::USABLE {
                        trace!("Locking (Used): {entry_range:#X?}");
                        let lock_start_index = entry_range.start / page_size();
                        let lock_end_index = entry_range.end / page_size();
                        bitmap.set(lock_start_index..lock_end_index).unwrap();
                    }

                    prev_entry_range_end = Some(entry_range.end);
                });

            debug!("Physical memory manager initialized.");

            Self {
                bitmap: RwLock::new(bitmap),
                total_frames,
            }
        });
    }

    fn get_static() -> &'static Self {
        PHYSICAL_MEMORY_MANAGER.get().unwrap()
    }

    fn with_bitmap<T>(func: impl FnOnce(&BitMap<'a>) -> T) -> T {
        Self::get_static().bitmap.with_shared(func)
    }

    fn with_bitmap_mut<T>(func: impl FnOnce(&mut BitMap<'a>) -> T) -> T {
        Self::get_static().bitmap.with_exclusive(func)
    }

    pub fn total_frames() -> usize {
        Self::get_static().total_frames
    }

    pub fn total_memory() -> usize {
        Self::total_frames() * page_size()
    }

    pub fn next_free(
        count: NonZero<usize>,
        clear_memory: bool,
    ) -> Result<Address<Frame>, NextFrameError> {
        Self::with_bitmap_mut(|bitmap| {
            let free_frame_index = bitmap.next_free(count).ok_or(NextFrameError::NoneFree)?;

            trace!(
                "Frames Locked: {:#X?}",
                free_frame_index..(free_frame_index + count.get())
            );

            let free_frame_address = free_frame_index << page_bits().get();
            let frame = Address::<Frame>::new(free_frame_address).unwrap();

            if clear_memory {
                // Safety: Memory was just allocated, and is not currently aliased.
                unsafe {
                    crate::mem::zero_frame(frame);
                }
            }

            Ok(frame)
        })
    }

    pub fn lock_frame(address: Address<Frame>) -> Result<(), FrameError> {
        Self::with_bitmap_mut(|bitmap| {
            let index = address.index();

            debug_assert!(bitmap.get(index)?);

            bitmap.set(index)?;

            trace!("Frame Locked: {:#X?}", index << page_bits().get());

            Ok(())
        })
    }

    pub fn free_frame(address: Address<Frame>) -> Result<(), FrameError> {
        Self::with_bitmap_mut(|bitmap| {
            let index = address.index();

            debug_assert!(!(bitmap.get(index)?));

            bitmap.unset(index)?;

            trace!("Freed: {:#X?}", index << page_bits().get());

            Ok(())
        })
    }

    pub fn is_locked(address: Address<Frame>) -> Result<bool, FrameError> {
        Self::with_bitmap(|bitmap| Ok(bitmap.get(address.index())?))
    }
}
