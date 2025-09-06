use crate::{
    mem::{
        HigherHalfDirectMap,
        addr::phys::{FrameAddress, HugeFrame, LargeFrame, StandardFrame},
    },
    util::{
        math::align_up_div,
        sync::{Once, RwLock},
    },
};
use core::{num::NonZero, ops::Range, ptr::NonNull};

mod segment;
use segment::Segment;

const SEGMENTS_PER_LARGE_PAGE: usize = 1usize
    << (LargeFrame::index_bit_shift().get()
        - Segment::INDEX_BITS_SHIFT
        - StandardFrame::index_bit_shift().get());

#[derive(Debug, Error)]
pub enum FrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,
}

unsafe fn zero_frame<F: FrameAddress>(frame: F) {
    let address = HigherHalfDirectMap::offset(frame.into());
    let ptr = NonNull::<u8>::with_exposed_provenance(address);
    unsafe {
        NonNull::write_bytes(ptr, 0, F::size_in_bytes());
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
                bitmap_region.start / StandardFrame::size_in_bytes(),
                bitmap_region.end / StandardFrame::size_in_bytes(),
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
                            prev_address_range.end / StandardFrame::size_in_bytes(),
                            address_range.start / StandardFrame::size_in_bytes(),
                        );
                    }

                    // Only lock the non-usable entries...
                    if memory_ty != limine::memory_map::EntryType::USABLE {
                        trace!("Locking (Used): {address_range:#X?}");
                        lock_bits(
                            bitmap,
                            address_range.start / StandardFrame::size_in_bytes(),
                            address_range.end / StandardFrame::size_in_bytes(),
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

            let total_frames =
                align_up_div(total_physical_memory, StandardFrame::index_bit_shift());
            trace!("Total frames: {total_frames} ({total_physical_memory:#X} Bytes)");

            // Aligned frame count to the next multiple of `usize`s bit count.
            let bitmap_size = align_up_div(
                total_frames,
                NonZero::new(usize::BITS.trailing_zeros()).unwrap(),
            );
            // Total memory the bitmap will consume as a multiple of frame size.
            let bitmap_size_in_frames = align_up_div(
                bitmap_size * core::mem::size_of::<usize>(),
                StandardFrame::index_bit_shift(),
            );
            // Total memory the bitmap will consume as a multiple of bytes.
            let bitmap_size_in_bytes = bitmap_size_in_frames * StandardFrame::size_in_bytes();

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

            debug_assert_eq!(bitmap_region.start & StandardFrame::non_index_bit_mask(), 0);
            debug_assert_eq!(bitmap_region.end & StandardFrame::non_index_bit_mask(), 0);

            trace!("Frame bitmap region: {bitmap_region:#X?}");

            let bitmap_address = HigherHalfDirectMap::offset(bitmap_region.start);
            let bitmap_ptr = NonNull::<u8>::with_exposed_provenance(bitmap_address);

            trace!("Zeroing bitmap...");
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                NonNull::write_bytes(bitmap_ptr, 0, bitmap_size_in_bytes);
            }

            trace!("Initializing bitmap...");
            let mut bitmap_ptr =
                NonNull::slice_from_raw_parts(bitmap_ptr.cast::<Segment>(), bitmap_size_in_bytes);

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

    pub fn next_free_frame<F: FrameAddress>(clear_memory: bool) -> Option<F> {
        let frame_index = match F::size_in_bytes() {
            size if size == StandardFrame::size_in_bytes() => {
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
                (segment_index << Segment::INDEX_BITS_SHIFT) | bit_index
            }

            size if size == LargeFrame::size_in_bytes() => {
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

                large_page_index << LargeFrame::index_bit_shift().get()
            }

            size if size == HugeFrame::size_in_bytes() => todo!(),

            _ => unreachable!(),
        };

        let frame = F::from_index(frame_index).ok()?;

        if clear_memory {
            unsafe {
                zero_frame(frame);
            }
        }

        Some(frame)
    }

    fn check_is_in_bounds<F: FrameAddress>(frame: F) -> Result<(), FrameError> {
        if (frame.index() + F::size_in_frames().get()) < Self::total_frames() {
            Ok(())
        } else {
            Err(FrameError::OutOfBounds)
        }
    }

    pub fn lock_frame<F: FrameAddress>(frame: F) -> Result<(), FrameError> {
        Self::check_is_in_bounds(frame)?;
        let index = frame.index();

        match F::size_in_bytes() {
            size if size == StandardFrame::size_in_bytes() => {
                let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
                let bit_index = index & Segment::INDEX_BITS_MASK;

                Self::with_bitmap_mut(|bitmap| {
                    // Safety: Index is checked to be within bounds.
                    let segment = unsafe { bitmap.get_unchecked_mut(segment_index) };
                    segment.unset_bit(bit_index);
                });

                Ok(())
            }

            size if size == LargeFrame::size_in_bytes() => {
                let segments_start_index = frame.index() * SEGMENTS_PER_LARGE_PAGE;
                Self::with_bitmap_mut(|bitmap| {
                    bitmap
                        .iter_mut()
                        .skip(segments_start_index)
                        .take(SEGMENTS_PER_LARGE_PAGE)
                        .for_each(Segment::set_empty);
                });

                Ok(())
            }

            size if size == HugeFrame::size_in_bytes() => todo!(),

            _ => unreachable!(),
        }
    }

    pub unsafe fn free_frame<F: FrameAddress>(frame: F) -> Result<(), FrameError> {
        Self::check_is_in_bounds(frame)?;
        let index = frame.index();

        match F::size_in_bytes() {
            size if size == StandardFrame::size_in_bytes() => {
                let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
                let bit_index = index & Segment::INDEX_BITS_MASK;

                Self::with_bitmap_mut(|bitmap| {
                    // Safety: Index is checked to be within bounds.
                    let segment = unsafe { bitmap.get_unchecked_mut(segment_index) };
                    segment.unset_bit(bit_index);
                });

                Ok(())
            }

            size if size == LargeFrame::size_in_bytes() => {
                let segments_start_index = frame.index() * SEGMENTS_PER_LARGE_PAGE;
                Self::with_bitmap_mut(|bitmap| {
                    bitmap
                        .iter_mut()
                        .skip(segments_start_index)
                        .take(SEGMENTS_PER_LARGE_PAGE)
                        .for_each(Segment::set_empty);
                });

                Ok(())
            }

            size if size == HugeFrame::size_in_bytes() => todo!(),

            _ => unreachable!(),
        }
    }

    pub fn is_locked<F: FrameAddress>(frame: F) -> Result<bool, FrameError> {
        Self::check_is_in_bounds(frame)?;

        let index = frame.index();
        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        Self::with_bitmap(|bitmap| {
            let segment = bitmap.get(segment_index).ok_or(FrameError::OutOfBounds)?;

            Ok(segment.get_bit(bit_index))
        })
    }
}
