use crate::{
    mem::HigherHalfDirectMap,
    util::sync::{Once, RwLock},
};
use core::{num::NonZero, ops::Range, ptr::NonNull};
use libsys::{
    address::{Address, Frame},
    constants::{
        huge_page_size, large_page_bits, large_page_size, page_bits, page_mask, page_size,
    },
    math::align_up_div,
};

mod segment;
use segment::Segment;

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,
}



unsafe fn zero_frame(frame: Address<Frame>, frame_size: FrameSize) {
    let address = HigherHalfDirectMap::offset(frame.get().get());
    let ptr = NonNull::<u8>::with_exposed_provenance(address);
    unsafe {
        NonNull::write_bytes(ptr, 0, frame_size.size_in_bytes());
    }
}

pub struct PhysicalMemoryManager {
    bitmap: RwLock<NonNull<[Segment]>>,
    total_frames: usize,
}

unsafe impl Send for PhysicalMemoryManager {}
unsafe impl Sync for PhysicalMemoryManager {}

static PHYSICAL_MEMORY_MANAGER: Once<PhysicalMemoryManager> = Once::new();

impl PhysicalMemoryManager {
    /// Initializes the static physical memory manager with the provided
    /// bootloader memory map request.
    pub fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        fn lock_bits(
            bitmap: &mut [Segment],
            start_bit_index_inclusive: usize,
            end_bit_index_exclusive: usize,
        ) {
            // OPTIMIZE This algorithm could be much faster by setting many bits at once.

            (start_bit_index_inclusive..end_bit_index_exclusive).for_each(|bit_index| {
                let segment_index = bit_index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
                let segment_bit_index = bit_index & Segment::INDEX_BITS_MASK;

                let segment = bitmap
                    .get_mut(segment_index)
                    .expect("`lock_bits` overran bitmap");
                segment.set_bit(segment_bit_index);
            });
        }

        fn init_bitmap_with_memory_map(
            memory_map: &[&limine::memory_map::Entry],
            bitmap: &mut [Segment],
            bitmap_region: Range<usize>,
        ) {
            // Ensure the bitmap's frames are reserved.
            trace!("Locking: {bitmap_region:#X?}");
            lock_bits(
                bitmap,
                bitmap_region.start / page_size(),
                bitmap_region.end / page_size(),
            );

            memory_map
                .iter()
                .map(|entry| {
                    // Map the entry to a usable range and type

                    let entry_start = usize::try_from(entry.base).unwrap();
                    let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                    (entry_start..entry_end, entry.entry_type)
                })
                .reduce(|(prev_address_range, _), (address_range, memory_ty)| {
                    // If there's space inbetween entries, we'll lock it to ensure it isn't
                    // accidentally used.
                    if prev_address_range.end < address_range.start {
                        trace!(
                            "Locking (Inbetween): {:#X?}",
                            prev_address_range.end..address_range.start
                        );
                        lock_bits(
                            bitmap,
                            prev_address_range.end / page_size(),
                            address_range.start / page_size(),
                        );
                    }

                    // Only lock the non-usable entries...
                    if memory_ty != limine::memory_map::EntryType::USABLE {
                        trace!("Locking (Used): {address_range:#X?}");
                        lock_bits(
                            bitmap,
                            address_range.start / page_size(),
                            address_range.end / page_size(),
                        );
                    }

                    (address_range, memory_ty)
                });
        }

        PHYSICAL_MEMORY_MANAGER.call_once(|| {
            trace!("Beginning Physical Memory Manager initialization...");

            let memory_map = memory_map_request
                .get_response()
                .expect("bootloader did not provide a response to the memory map request")
                .entries();

            let last_entry = memory_map.last().unwrap();
            // While this is the ""total"" physical memory, it should be noted it isn't the
            // total *installed* memory. Because of hardware addressing, reserved
            // // regions—and other quirks—this number will likely be much larger than the
            // actual amount of installed physical memory the machine has.
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

            let bitmap_address = HigherHalfDirectMap::offset(bitmap_region.start);
            let bitmap_ptr = NonNull::<u8>::with_exposed_provenance(bitmap_address);

            trace!("Zeroing bitmap...");
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                NonNull::write_bytes(bitmap_ptr, 0, bitmap_size_in_bytes);
            }

            trace!("Initializing bitmap...");
            let mut bitmap_ptr = unsafe {
                NonNull::slice_from_raw_parts(bitmap_ptr.cast::<Segment>(), bitmap_size_in_bytes)
            };

            init_bitmap_with_memory_map(memory_map, unsafe { bitmap_ptr.as_mut() }, bitmap_region);
            trace!("Bitmap fully initialized.");

            debug_assert!(total_frames <= (bitmap_ptr.len() * size_of::<usize>()));

            debug!("Physical memory manager initialized.");

            Self {
                bitmap: RwLock::new(bitmap_ptr),
                total_frames,
            }
        });
    }

    fn with_bitmap<T>(func: impl FnOnce(&[Segment]) -> T) -> T {
        PHYSICAL_MEMORY_MANAGER
            .wait()
            .bitmap
            .with_shared(|bitmap_ptr| {
                let bitmap = unsafe { bitmap_ptr.as_ref() };
                func(bitmap)
            })
    }

    fn with_bitmap_mut<T>(func: impl FnOnce(&mut [Segment]) -> T) -> T {
        PHYSICAL_MEMORY_MANAGER
            .wait()
            .bitmap
            .with_exclusive(|bitmap_ptr| {
                let bitmap = unsafe { bitmap_ptr.as_mut() };
                func(bitmap)
            })
    }

    pub fn total_frames() -> usize {
        PHYSICAL_MEMORY_MANAGER.wait().total_frames
    }

    pub fn next_free_frame(frame_size: FrameSize, clear_memory: bool) -> Option<Address<Frame>> {
        let frame = match frame_size {
            FrameSize::Standard => {
                let (segment_index, bit_index) = Self::with_bitmap_mut(|bitmap| {
                    bitmap
                        .iter_mut()
                        .enumerate()
                        .find_map(|(segment_index, segment)| {
                            segment
                                .next_free()
                                .map(|bit_index| (segment_index, bit_index))
                        })
                })?;

                let bit_index = usize::try_from(bit_index).unwrap();
                let frame_index = (segment_index << Segment::INDEX_BITS_SHIFT) | bit_index;
                Address::<Frame>::from_index(frame_index).unwrap()
            }

            FrameSize::Large => {
                const SEGMENTS_PER_LARGE_PAGE: u32 =
                    large_page_bits().get() - Segment::INDEX_BITS_SHIFT - page_bits().get();

                let large_page_index = Self::with_bitmap_mut(|bitmap| {
                    #[allow(clippy::as_conversions)]
                    let (large_page_segment_chunks, _) =
                        bitmap.as_chunks_mut::<{ SEGMENTS_PER_LARGE_PAGE as usize }>();

                    let (large_page_index, large_page_segment_chunk) = large_page_segment_chunks
                        .iter_mut()
                        .enumerate()
                        .find(|(_, segment_chunk)| {
                            segment_chunk.iter().all(|segment| segment.is_empty())
                        })?;

                    large_page_segment_chunk.fill(Segment::FULL);

                    Some(large_page_index)
                })?;

                let frame_index = large_page_index << large_page_bits().get();
                Address::<Frame>::new(frame_index).unwrap()
            }

            FrameSize::Huge => todo!(),
        };

        if clear_memory {
            unsafe {
                zero_frame(frame, frame_size);
            }
        }

        Some(frame)
    }

    pub fn lock_frame(address: Address<Frame>, frame_size: FrameSize) -> Result<(), FrameError> {
        let index = address.index();
        if index > Self::total_frames() {
            return Err(FrameError::OutOfBounds);
        }

        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        Self::with_bitmap_mut(|bitmap| {
            // Safety: Index is checked to be within bounds.
            let segment = unsafe { bitmap.get_mut(segment_index).unwrap_unchecked() };
            segment.set_bit(bit_index);

            Ok(())
        })
    }

    pub unsafe fn free_frame(
        address: Address<Frame>,
        frame_size: FrameSize,
    ) -> Result<(), FrameError> {
        let index = address.index();
        if index > Self::total_frames() {
            return Err(FrameError::OutOfBounds);
        }

        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        Self::with_bitmap_mut(|bitmap| {
            // Safety: Index is checked to be within bounds.
            let segment = unsafe { bitmap.get_mut(segment_index).unwrap_unchecked() };
            segment.unset_bit(bit_index);

            Ok(())
        })
    }

    pub fn is_locked(address: Address<Frame>) -> Result<bool, FrameError> {
        let index = address.index();
        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        Self::with_bitmap(|bitmap| {
            let segment = bitmap.get(segment_index).ok_or(FrameError::OutOfBounds)?;

            Ok(segment.get_bit(bit_index))
        })
    }
}
