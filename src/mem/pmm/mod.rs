use crate::{
    mem::{
        addr::phys::{FrameAddress, HugeFrame, LargeFrame, PhysicalAddress, StandardFrame},
        pmm::segment::{SegmentRepr, SEGMENT_BITS_USIZE},
        HigherHalfDirectMap,
    },
    util::{
        math::align_up_div,
        sync::{Once, RwLock},
    },
};
use core::{num::NonZero, ops::Range, ptr::NonNull};
use limine::memory_map::{Entry as MmapEntry, EntryType as MmapEntryType};

mod segment;
use segment::Segment;

#[derive(Debug, Clone, PartialEq, Eq)]
struct BitmapSize {
    pub total_memory: usize,
    pub total_frames: usize,
    pub size_in_frames: usize,
    pub size_in_bytes: usize,
}

/// # Remarks
///
/// - `memory_map` is an iterator to allow easier testing.
#[allow(clippy::as_conversions)]
fn calculate_bitmap_size_from_memory_map(last_memory_map_entry: &MmapEntry) -> BitmapSize {
    // While this is the ""total"" physical memory, it should be noted it isn't the
    // total *installed* memory. Because of hardware addressing, reserved
    // regions—and other quirks—this number will likely be much larger than the
    // actual amount of installed physical memory the machine has.
    let total_memory =
        usize::try_from(last_memory_map_entry.base + last_memory_map_entry.length).unwrap();
    let total_frames = align_up_div(total_memory, StandardFrame::INDEX_BIT_SHIFT);

    // Total memory the bitmap will consume as a multiple of frame size.
    let size_in_bytes = align_up_div(total_frames, {
        // Safety: Value is non-zero.
        unsafe { NonZero::<u32>::new_unchecked(u8::BITS.trailing_zeros()) }
    });

    // Total memory the bitmap will consume as a multiple of segments.
    let size_in_frames = align_up_div(size_in_bytes, StandardFrame::INDEX_BIT_SHIFT);

    BitmapSize {
        total_memory,
        total_frames,
        size_in_frames,
        size_in_bytes,
    }
}

/// Decomposes a raw bit index into its segment index part (`usize`) and its
/// segment bit index part (`u32`).
fn decompose_bit_index(bit_index: usize) -> (usize, u32) {
    let segment_index = bit_index.unbounded_shr(Segment::INDEX_BITS_SHIFT.get());
    let segment_bit_index = bit_index & Segment::INDEX_BITS_MASK.get();
    let segment_bit_index = u32::try_from(segment_bit_index).unwrap();

    (segment_index, segment_bit_index)
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum LockFrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,

    #[error("attempted to lock frame that was already locked")]
    NotAllFree,
}

impl From<FrameError> for LockFrameError {
    fn from(error: FrameError) -> Self {
        match error {
            FrameError::OutOfBounds => Self::OutOfBounds,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum FreeFrameError {
    #[error("attempted to index out of bounds")]
    OutOfBounds,

    #[error("attempted to free frame that was already free")]
    NotAllLocked,
}

impl From<FrameError> for FreeFrameError {
    fn from(error: FrameError) -> Self {
        match error {
            FrameError::OutOfBounds => Self::OutOfBounds,
        }
    }
}

pub struct PhysicalMemoryManager(RwLock<PhysicalMemoryManagerInner<'static>>);

// Safety: Inner data is contained in a safe interior-mutability primitive.
unsafe impl Sync for PhysicalMemoryManager {}

static PHYSICAL_MEMORY_MANAGER: Once<PhysicalMemoryManager> = Once::new();

impl PhysicalMemoryManager {
    pub const BLOCK_SIZE: NonZero<usize> = SEGMENT_BITS_USIZE
        .checked_mul(StandardFrame::SIZE_IN_BYTES)
        .unwrap();

    /// Initializes the static physical memory manager with the provided
    /// bootloader memory map request.
    ///
    /// # Safety
    ///
    /// - `memory_map_request` must not have been allocated from memory since
    ///   kernel entry.
    pub unsafe fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        // TODO Guarantee that the bitmap size will not exceed the maximum addressable
        // physical memory space.

        let memory_map = memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries();

        PHYSICAL_MEMORY_MANAGER.call_once(|| {
            trace!("Beginning Physical Memory Manager initialization...");

            fn lock_bits(
                bitmap: &mut [Segment],
                start_bit_index_inclusive: usize,
                end_bit_index_exclusive: usize,
            ) {
                debug_assert!(start_bit_index_inclusive < end_bit_index_exclusive);

                let (mut segment_index, mut segment_bit) =
                    decompose_bit_index(start_bit_index_inclusive);
                let mut remaining_bits = end_bit_index_exclusive - start_bit_index_inclusive;

                // This loop should run *at most* 3 times:
                // 1. For the initial bits in an imperfectly offset start bit.
                // 2. For the memset on all of the middle segments.
                // 3. For the remaining bits in an imperfectly offset end bit.
                while remaining_bits > 0 {
                    if segment_bit == 0 && remaining_bits > SEGMENT_BITS_USIZE.get() {
                        // Doing a memset for the full segment runs is much faster.

                        let remaining_full_segments = remaining_bits / SEGMENT_BITS_USIZE.get();
                        bitmap
                            .get_mut(segment_index..(segment_index + remaining_full_segments))
                            .expect("overran bitmap in `lock_bits` (multi-set)")
                            .fill(Segment::FULL);

                        remaining_bits -= SEGMENT_BITS_USIZE.get() * remaining_full_segments;
                        segment_index += remaining_full_segments;
                    } else {
                        let remaining_bits_for_segment = core::cmp::min(
                            Segment::BITS.get() - segment_bit,
                            u32::try_from(remaining_bits).unwrap_or(u32::MAX),
                        );
                        let high_bit_shift = Segment::BITS.get() - remaining_bits_for_segment;
                        let low_bit_shift = segment_bit;

                        let mask = SegmentRepr::MAX
                            .unbounded_shr(low_bit_shift)
                            .unbounded_shl(high_bit_shift)
                            .strict_shr(high_bit_shift - low_bit_shift);

                        *bitmap
                            .get_mut(segment_index)
                            .expect("overran bitmap in `lock_bits` (single-set)")
                            .inner_mut() |= mask;

                        remaining_bits -= usize::try_from(remaining_bits_for_segment).unwrap();
                        segment_index += 1;
                    }

                    segment_bit = 0;
                }
            }

            fn init_bitmap_with_memory_map(
                memory_map: &[&MmapEntry],
                bitmap: &mut [Segment],
                bitmap_region: Range<usize>,
            ) {
                // Ensure the bitmap's frames are reserved.
                debug!("Locking (BITMAP): {bitmap_region:#X?}");
                lock_bits(
                    bitmap,
                    bitmap_region.start / StandardFrame::SIZE_IN_BYTES.get(),
                    bitmap_region.end / StandardFrame::SIZE_IN_BYTES.get(),
                );

                memory_map
                    .iter()
                    .map(|entry| {
                        // Map the entry to a usable range and type

                        let entry_start = usize::try_from(entry.base).unwrap();
                        let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                        (entry_start..entry_end, entry.entry_type)
                    })
                    .fold(
                        Option::<(Range<usize>, MmapEntryType)>::None,
                        |prev_entry, (address_range, memory_ty)| {
                            if let Some((prev_address_range, prev_memory_ty)) = prev_entry {
                                if prev_address_range.end < address_range.start {
                                    // If there's space between entries, we'll lock it to ensure it
                                    // isn't accidentally used.

                                    debug!(
                                        "Locking (BETWEEN): {:#X?}",
                                        prev_address_range.end..address_range.start
                                    );
                                    lock_bits(
                                        bitmap,
                                        prev_address_range.end / StandardFrame::SIZE_IN_BYTES.get(),
                                        address_range.start / StandardFrame::SIZE_IN_BYTES.get(),
                                    );
                                } else if memory_ty == prev_memory_ty
                                    && address_range.start == prev_address_range.end
                                {
                                    // Coalesce entries to reduce work.

                                    return Some((
                                        prev_address_range.start..address_range.end,
                                        memory_ty,
                                    ));
                                }
                            }

                            // Only lock the non-usable entries...
                            if memory_ty != MmapEntryType::USABLE {
                                debug!("Locking (USED): {address_range:#X?}");
                                lock_bits(
                                    bitmap,
                                    address_range.start / StandardFrame::SIZE_IN_BYTES.get(),
                                    address_range.end / StandardFrame::SIZE_IN_BYTES.get(),
                                );
                            }

                            Some((address_range, memory_ty))
                        },
                    );
            }

            let last_memory_map_entry = memory_map.last().unwrap();
            let bitmap_size = calculate_bitmap_size_from_memory_map(last_memory_map_entry);
            trace!("{bitmap_size:#X?}");

            // Select a region that will fit the bitmap, aligned to frame size.
            let bitmap_region = memory_map
                .iter()
                .filter(|entry| entry.entry_type == MmapEntryType::USABLE)
                .find_map(|entry| {
                    let entry_start = usize::try_from(entry.base).unwrap();
                    let entry_length = usize::try_from(entry.length).unwrap();

                    if entry_length >= bitmap_size.size_in_bytes {
                        Some(entry_start..(entry_start + bitmap_size.size_in_bytes))
                    } else {
                        None
                    }
                })
                .expect("no memory regions large enough for frame bitmap");

            debug_assert_eq!(
                bitmap_region.start & StandardFrame::NON_INDEX_BIT_MASK.get(),
                0
            );
            debug_assert_eq!(
                bitmap_region.end & StandardFrame::NON_INDEX_BIT_MASK.get(),
                0
            );

            trace!("Frame bitmap region: {bitmap_region:#X?}");

            trace!("Initializing bitmap...");
            let bitmap = {
                let bitmap_address = HigherHalfDirectMap::offset(bitmap_region.start);
                let ptr = NonNull::<u8>::with_exposed_provenance(bitmap_address);

                trace!("Zeroing bitmap...");
                // Safety: Caller is required to maintain safety invariants.
                unsafe {
                    NonNull::write_bytes(ptr, 0, bitmap_size.size_in_bytes);
                }

                debug_assert_eq!(bitmap_size.size_in_bytes % size_of::<Segment>(), 0);

                let mut ptr = NonNull::slice_from_raw_parts(
                    ptr.cast::<Segment>(),
                    bitmap_size.size_in_bytes / size_of::<Segment>(),
                );

                // Safety:
                // Constructor required that no memory map entires be in use, and the memory map
                // response guarantees that entries marked `USABLE` will not be currently
                // aliased.
                unsafe { ptr.as_mut() }
            };

            init_bitmap_with_memory_map(memory_map, bitmap, bitmap_region);

            trace!("Bitmap fully initialized.");

            // Safety: `bitmap_size.total_frames` is calculated to be correct for `bitmap`.
            let inner = PhysicalMemoryManagerInner::new(bitmap, bitmap_size.total_frames);
            Self(RwLock::new(inner))
        });
    }

    pub fn total_frames() -> usize {
        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_shared(PhysicalMemoryManagerInner::total_frames)
    }

    pub fn is_any_locked<F: FrameAddress>(frame: F) -> Result<bool, FrameError> {
        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_shared(|inner| inner.is_any_locked(frame))
    }

    pub fn lock_frame<F: FrameAddress>(frame: F) -> Result<(), LockFrameError> {
        trace!(
            "Lock ({:#X}): {:X?}",
            F::SIZE_IN_BYTES.get(),
            Into::<PhysicalAddress>::into(frame)
        );

        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_exclusive(|inner| inner.lock_frame(frame))
    }

    pub unsafe fn free_frame<F: FrameAddress>(frame: F) -> Result<(), FreeFrameError> {
        trace!(
            "Free ({:#X}): {:X?}",
            F::SIZE_IN_BYTES.get(),
            Into::<PhysicalAddress>::into(frame)
        );

        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_exclusive(|inner| {
                // Safety: Caller is required to maintain safety invariants.
                unsafe { inner.free_frame(frame) }
            })
    }

    pub fn next_free_frame<F: FrameAddress>() -> Option<F> {
        trace!(
            "Next Free Frame ({:#X}): Allocating",
            F::SIZE_IN_BYTES.get()
        );

        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_exclusive(PhysicalMemoryManagerInner::next_free_frame)
            .inspect(|frame| {
                trace!(
                    "Next Free Frame ({:#X}): {:X?}",
                    F::SIZE_IN_BYTES.get(),
                    Into::<PhysicalAddress>::into(*frame)
                );
            })
    }

    /// Searches for a free run of segments.
    pub fn next_free_segments(count: NonZero<usize>) -> Option<Range<StandardFrame>> {
        trace!(
            "Next Free Segments: {{ Size:{:#X} }}",
            count.get() * StandardFrame::SIZE_IN_BYTES.get()
        );

        PHYSICAL_MEMORY_MANAGER
            .get()
            .unwrap()
            .0
            .with_exclusive(|inner| inner.next_free_segments(count))
            .inspect(|frames| {
                trace!("Next Free Segments: {frames:X?}");
            })
    }
}

macro_rules! func_by_frame_size {
    (
        $standard_func:block,
        $large_func:block,
        $huge_func:block
    ) => {
        match F::SIZE_IN_BYTES {
            StandardFrame::SIZE_IN_BYTES => $standard_func,
            LargeFrame::SIZE_IN_BYTES => $large_func,
            HugeFrame::SIZE_IN_BYTES => $huge_func,

            _ => unreachable!(),
        }
    };
}

#[derive(Debug, PartialEq, Eq)]
struct PhysicalMemoryManagerInner<'a> {
    bitmap: &'a mut [Segment],
    total_frames: usize,
}

impl<'a> PhysicalMemoryManagerInner<'a> {
    pub const BLOCK_SIZE: NonZero<usize> = SEGMENT_BITS_USIZE
        .checked_mul(StandardFrame::SIZE_IN_BYTES)
        .unwrap();

    /// Initializes the static physical memory manager with the provided
    /// bootloader memory map request.
    pub fn new(bitmap: &'a mut [Segment], total_frames: usize) -> Self {
        let max_frames = bitmap.len() * SEGMENT_BITS_USIZE.get();

        assert!(
            total_frames <= max_frames,
            "`total_frames` ({total_frames}) cannot exceed the bit-width of `bitmap` ({max_frames})"
        );

        if max_frames > total_frames {
            // Fill in the extant bits so they are not locked by the `next_free_` functions.

            let segment_index = total_frames / SEGMENT_BITS_USIZE;
            let start_segment_bit = u32::try_from(total_frames % SEGMENT_BITS_USIZE).unwrap();
            let mask_bit_count = SegmentRepr::BITS - start_segment_bit;
            let segment_mask = SegmentRepr::unbounded_shl(1, mask_bit_count).wrapping_sub(1);

            debug!(
                "Extant Fill: {{ Total Bits: {:#X}, Total Segments: {}, Start Segment: {segment_index}, Start Segment Bit: {start_segment_bit}, Mask Bit Count: {mask_bit_count}, Mask: {segment_mask:#b} }}",
                max_frames - total_frames,
                (max_frames - total_frames) / SEGMENT_BITS_USIZE
            );

            bitmap[segment_index] = Segment::new(segment_mask << start_segment_bit);
            if let Some(segments) = bitmap.get_mut((segment_index + 1)..) {
                segments.fill(Segment::FULL);
            }
        }

        Self {
            bitmap,
            total_frames,
        }
    }

    pub fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn check_is_in_bounds<F: FrameAddress>(&self, frame: F) -> bool {
        (frame.index() + F::SIZE_IN_FRAMES.get()) < self.total_frames
    }

    pub fn is_any_locked<F: FrameAddress>(&self, frame: F) -> Result<bool, FrameError> {
        if !self.check_is_in_bounds(frame) {
            return Err(FrameError::OutOfBounds);
        }

        let is_locked = {
            func_by_frame_size!(
                {
                    let bit_index = frame.index();
                    let (segment_index, segment_bit_index) = decompose_bit_index(bit_index);

                    // Safety: Index has is checked within bounds.
                    unsafe {
                        self.bitmap
                            .get_unchecked(segment_index)
                            .get_bit(segment_bit_index)
                    }
                },
                {
                    let segments_start_index = frame.index() * Segment::PER_LARGE_PAGE.get();
                    // Safety: Index range is checked to be within bounds.
                    let segment_chunk = unsafe {
                        self.bitmap.get_unchecked(
                            segments_start_index
                                ..(segments_start_index + Segment::PER_LARGE_PAGE.get()),
                        )
                    };

                    segment_chunk != [Segment::EMPTY; Segment::PER_LARGE_PAGE.get()]
                },
                { todo!() }
            )
        };

        Ok(is_locked)
    }

    pub fn is_all_locked<F: FrameAddress>(&self, frame: F) -> Result<bool, FrameError> {
        if !self.check_is_in_bounds(frame) {
            return Err(FrameError::OutOfBounds);
        }

        let is_locked = {
            func_by_frame_size!(
                {
                    let bit_index = frame.index();
                    let (segment_index, segment_bit_index) = decompose_bit_index(bit_index);

                    // Safety: Index has is checked within bounds.
                    unsafe {
                        self.bitmap
                            .get_unchecked(segment_index)
                            .get_bit(segment_bit_index)
                    }
                },
                {
                    let segments_start_index = frame.index() * Segment::PER_LARGE_PAGE.get();
                    // Safety: Index range is checked to be within bounds.
                    let segment_chunk = unsafe {
                        self.bitmap.get_unchecked(
                            segments_start_index
                                ..(segments_start_index + Segment::PER_LARGE_PAGE.get()),
                        )
                    };

                    segment_chunk == [Segment::FULL; Segment::PER_LARGE_PAGE.get()]
                },
                { todo!() }
            )
        };

        Ok(is_locked)
    }

    pub fn is_all_free<F: FrameAddress>(&self, frame: F) -> Result<bool, FrameError> {
        if !self.check_is_in_bounds(frame) {
            return Err(FrameError::OutOfBounds);
        }

        let is_locked = {
            func_by_frame_size!(
                {
                    let bit_index = frame.index();
                    let (segment_index, segment_bit_index) = decompose_bit_index(bit_index);

                    // Safety: Index has is checked within bounds.
                    unsafe {
                        !self
                            .bitmap
                            .get_unchecked(segment_index)
                            .get_bit(segment_bit_index)
                    }
                },
                {
                    let segments_start_index = frame.index() * Segment::PER_LARGE_PAGE.get();
                    // Safety: Index range is checked to be within bounds.
                    let segment_chunk = unsafe {
                        self.bitmap.get_unchecked(
                            segments_start_index
                                ..(segments_start_index + Segment::PER_LARGE_PAGE.get()),
                        )
                    };

                    segment_chunk == [Segment::EMPTY; Segment::PER_LARGE_PAGE.get()]
                },
                { todo!() }
            )
        };

        Ok(is_locked)
    }

    pub fn lock_frame<F: FrameAddress>(&mut self, frame: F) -> Result<(), LockFrameError> {
        if !(self.is_all_free(frame)?) {
            return Err(LockFrameError::NotAllFree);
        }

        func_by_frame_size!(
            {
                let (segment_index, segment_bit_index) = decompose_bit_index(frame.index());
                // Safety: Index is checked to be within bounds.
                unsafe {
                    self.bitmap
                        .get_unchecked_mut(segment_index)
                        .set_bit(segment_bit_index);
                }
            },
            {
                let segments_start_index = frame.index() * Segment::PER_LARGE_PAGE.get();
                // Safety: Index range is checked to be within bounds.
                unsafe {
                    self.bitmap
                        .get_unchecked_mut(
                            segments_start_index
                                ..(segments_start_index + Segment::PER_LARGE_PAGE.get()),
                        )
                        .fill(Segment::FULL);
                }
            },
            { todo!() }
        );

        Ok(())
    }

    pub unsafe fn free_frame<F: FrameAddress>(&mut self, frame: F) -> Result<(), FreeFrameError> {
        if !(self.is_all_locked(frame)?) {
            return Err(FreeFrameError::NotAllLocked);
        }

        func_by_frame_size!(
            {
                let (segment_index, segment_bit_index) = decompose_bit_index(frame.index());

                // Safety: Index is checked to be within bounds.
                let segment = unsafe { self.bitmap.get_unchecked_mut(segment_index) };

                segment.unset_bit(segment_bit_index);
            },
            {
                let segments_start_index = frame.index() * Segment::PER_LARGE_PAGE.get();

                // Safety: Index range is checked to be within bounds.
                let segment_chunk = unsafe {
                    self.bitmap.get_unchecked_mut(
                        segments_start_index
                            ..(segments_start_index + Segment::PER_LARGE_PAGE.get()),
                    )
                };

                segment_chunk.fill(Segment::EMPTY);
            },
            { todo!() }
        );

        Ok(())
    }

    pub fn next_free_frame<F: FrameAddress>(&mut self) -> Option<F> {
        let frame_index =
            func_by_frame_size!(
                {
                    let (segment_index, bit_index) = self.bitmap.iter_mut().enumerate().find_map(
                        |(segment_index, segment)| {
                            segment
                                .next_free()
                                .map(|bit_index| (segment_index, bit_index))
                        },
                    )?;

                    let bit_index = usize::try_from(bit_index).unwrap();
                    (segment_index << Segment::INDEX_BITS_SHIFT.get()) | bit_index
                },
                {
                    let large_page_index = self
                        .bitmap
                        .chunks_exact_mut(Segment::PER_LARGE_PAGE.get())
                        .enumerate()
                        .find_map(|(large_page_index, segment_chunk)| {
                            if segment_chunk == [Segment::EMPTY; Segment::PER_LARGE_PAGE.get()] {
                                segment_chunk.fill(Segment::FULL);
                                Some(large_page_index)
                            } else {
                                None
                            }
                        })?;

                    const LARGE_PAGE_TO_STANDARD_INDEX_SHIFT: u32 =
                        LargeFrame::INDEX_BIT_SHIFT.get() - StandardFrame::INDEX_BIT_SHIFT.get();

                    large_page_index << LARGE_PAGE_TO_STANDARD_INDEX_SHIFT
                },
                { todo!() }
            );

        // Safety:
        // `frame_index` is derived from a bit index, and `Self` requires that `bitmap`s
        // bit size not exceed canonical physical address space.
        let frame = unsafe { F::from_index(frame_index).unwrap_unchecked() };

        Some(frame)
    }

    pub fn next_free_segments(&mut self, count: NonZero<usize>) -> Option<Range<StandardFrame>> {
        let frame_index = {
            let mut windows = self.bitmap.windows(count.get()).enumerate();

            loop {
                let (frame_index, window) = windows.next()?;
                if window.iter().all(|segment| segment.is_empty()) {
                    break Some(frame_index);
                }

                let skip = window
                    .iter()
                    .enumerate()
                    .rfind(|(_, segment)| !segment.is_empty())
                    .map(|(skip, _)| skip);

                // Safety: We know at least one segment was `!is_empty()`, because if not we
                // would have returned the window index as a success.
                let skip = unsafe { skip.unwrap_unchecked() };

                windows.advance_by(skip).ok()?;
            }
        }?;

        // Safety: `frame_index` is inherently in-bounds, because it is derived from a
        // `.enumerate()` on a `.windows()` call.
        let frame_window = unsafe {
            self.bitmap
                .get_unchecked_mut(frame_index..(frame_index + count.get()))
        };
        frame_window.fill(Segment::FULL);

        // Safety: Indexes are from bitmap, so guaranteed to be valid.
        let (start_frame, end_frame) = unsafe {
            (
                StandardFrame::from_index(frame_index).unwrap_unchecked(),
                StandardFrame::from_index(frame_index + count.get()).unwrap_unchecked(),
            )
        };

        Some(start_frame..end_frame)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::as_conversions, clippy::cast_possible_truncation)]

    use super::{BitmapSize, FreeFrameError, LockFrameError, PhysicalMemoryManagerInner, Segment};
    use crate::mem::{
        addr::phys::{FrameAddress, LargeFrame, StandardFrame},
        pmm::segment::{SegmentRepr, SEGMENT_BITS_USIZE},
    };
    use limine::memory_map::{Entry, EntryType};

    const MEMORY_MAP_LEN: usize = 8;
    const fn new_memory_map() -> [Entry; MEMORY_MAP_LEN] {
        [
            Entry {
                base: 0,
                length: 0x2000,
                entry_type: EntryType::USABLE,
            },
            Entry {
                base: 0x3000,
                length: 0x10_0000,
                entry_type: EntryType::RESERVED,
            },
            Entry {
                base: 0x10_3000,
                length: 0x100_0000,
                entry_type: EntryType::USABLE,
            },
            Entry {
                base: 0x2000_0000,
                length: 0x10000,
                entry_type: EntryType::BAD_MEMORY,
            },
            Entry {
                base: 0x2001_0000,
                length: 0x2000,
                entry_type: EntryType::EXECUTABLE_AND_MODULES,
            },
            Entry {
                base: 0x3001_0000,
                length: 0x10_0000,
                entry_type: EntryType::USABLE,
            },
            Entry {
                base: 0x4001_0000,
                length: 0x10_0000,
                entry_type: EntryType::FRAMEBUFFER,
            },
            Entry {
                base: 0x6001_0000,
                length: 0x10_0000,
                entry_type: EntryType::USABLE,
            },
        ]
    }

    #[test]
    fn calculate_bitmap_sizes() {
        assert_eq!(
            super::calculate_bitmap_size_from_memory_map(&(new_memory_map()[MEMORY_MAP_LEN - 1])),
            BitmapSize {
                total_memory: 0x60110000,
                total_frames: 0x60110,
                size_in_frames: 0xD,
                size_in_bytes: 0xC022,
            }
        );
    }

    #[test]
    fn decompose_bit_index() {
        const TEST_BIT_INDEX: usize = 1234567869;
        assert_eq!(
            super::decompose_bit_index(TEST_BIT_INDEX),
            (
                TEST_BIT_INDEX >> SegmentRepr::BITS.trailing_zeros(),
                (TEST_BIT_INDEX as u32) & ((1u32 << SegmentRepr::BITS.trailing_zeros()) - 1)
            )
        );
    }

    const USABLE_SEGMENTS: usize = Segment::PER_LARGE_PAGE.get() * 2;
    const USABLE_BITMAP_BITS: usize = USABLE_SEGMENTS * SEGMENT_BITS_USIZE.get();
    const BITMAP_SEGMENTS_LEN: usize = USABLE_SEGMENTS + 1;
    fn new_bitmap() -> [Segment; BITMAP_SEGMENTS_LEN] {
        let mut bitmap = core::array::repeat::<Segment, BITMAP_SEGMENTS_LEN>(Segment::EMPTY);
        bitmap[0..Segment::PER_LARGE_PAGE.get()].fill(Segment::FULL);
        bitmap[Segment::PER_LARGE_PAGE.get() - 1] =
            Segment::new(SegmentRepr::MAX & !(1 << (Segment::BITS.get() - 1)));

        bitmap[bitmap.len() - 1] = Segment::FULL;

        // TODO Test huge-pages as well.
        bitmap
    }

    #[test]
    fn new() {
        let mut bitmap_1 = new_bitmap();
        let mut bitmap_2 = new_bitmap();
        let pmm = PhysicalMemoryManagerInner::new(&mut bitmap_1, USABLE_BITMAP_BITS);
        assert_eq!(
            pmm,
            PhysicalMemoryManagerInner {
                bitmap: &mut bitmap_2,
                total_frames: USABLE_BITMAP_BITS
            }
        );
    }

    #[test]
    fn is_any_locked() {
        let mut subject_bitmap = new_bitmap();
        let pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        assert!(pmm
            .is_any_locked(StandardFrame::from_index(449).unwrap())
            .unwrap());
        assert!(pmm
            .is_any_locked(LargeFrame::from_index(0).unwrap())
            .unwrap());
        assert!(!pmm
            .is_any_locked(LargeFrame::from_index(1).unwrap())
            .unwrap());

        // TODO test huge
    }

    #[test]
    fn is_all_locked() {
        let mut subject_bitmap = new_bitmap();
        let pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        assert!(pmm
            .is_all_locked(StandardFrame::from_index(449).unwrap())
            .unwrap());
        assert!(!pmm
            .is_all_locked(LargeFrame::from_index(0).unwrap())
            .unwrap());
        assert!(!pmm
            .is_all_locked(LargeFrame::from_index(1).unwrap())
            .unwrap());

        // TODO test huge
    }

    #[test]
    fn is_all_free() {
        let mut subject_bitmap = new_bitmap();
        let pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        assert!(!pmm
            .is_all_free(StandardFrame::from_index(449).unwrap())
            .unwrap());
        assert!(!pmm.is_all_free(LargeFrame::from_index(0).unwrap()).unwrap());
        assert!(pmm.is_all_free(LargeFrame::from_index(1).unwrap()).unwrap());

        // TODO test huge
    }

    #[test]
    fn lock_frame() {
        let mut subject_bitmap = new_bitmap();
        let lock_standard_bitmap = {
            let mut b = new_bitmap();
            b[Segment::PER_LARGE_PAGE.get()] = Segment::new(1);
            b
        };

        let mut pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        let lock_standard_frame = StandardFrame::from_index(512).unwrap();
        assert_eq!(pmm.lock_frame(lock_standard_frame), Ok(()));
        assert_eq!(pmm.bitmap, lock_standard_bitmap);

        let lock_large_frame = LargeFrame::from_index(1).unwrap();
        assert_eq!(
            pmm.lock_frame(lock_large_frame),
            Err(LockFrameError::NotAllFree)
        );
        assert_eq!(pmm.bitmap, lock_standard_bitmap);
    }

    #[test]
    fn free_frame() {
        let mut subject_bitmap = {
            let mut b = new_bitmap();
            b[Segment::PER_LARGE_PAGE.get() - 1] = Segment::FULL;
            b[Segment::PER_LARGE_PAGE.get()] = Segment::new(1);
            b
        };
        let free_large_bitmap = {
            let mut b = new_bitmap();
            b[0..Segment::PER_LARGE_PAGE.get()].fill(Segment::EMPTY);
            b[Segment::PER_LARGE_PAGE.get()] = Segment::new(1);
            b
        };
        let free_standard_bitmap = new_bitmap();

        let mut pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        let free_large_frame = LargeFrame::from_index(0).unwrap();
        debug_assert_eq!(unsafe { pmm.free_frame(free_large_frame) }, Ok(()));
        debug_assert_eq!(pmm.bitmap, free_large_bitmap);

        let free_large_frame = LargeFrame::from_index(1).unwrap();
        debug_assert_eq!(
            unsafe { pmm.free_frame(free_large_frame) },
            Err(FreeFrameError::NotAllLocked)
        );
        debug_assert_eq!(pmm.bitmap, free_large_bitmap);

        let free_standard_frame = StandardFrame::from_index(512).unwrap();
        debug_assert_eq!(unsafe { pmm.free_frame(free_standard_frame) }, Ok(()));
        debug_assert_eq!(pmm.bitmap, {
            let mut b = [Segment::EMPTY; BITMAP_SEGMENTS_LEN];
            b[b.len() - 1] = Segment::FULL;
            b
        });
    }

    #[test]
    fn next_free_frame() {
        let mut subject_bitmap = new_bitmap();
        let next_free_standard_bitmap = {
            let mut b = new_bitmap();
            b[Segment::PER_LARGE_PAGE.get() - 1] = Segment::FULL;
            b
        };
        let next_free_large_bitmap = [Segment::FULL; BITMAP_SEGMENTS_LEN];

        let mut pmm = PhysicalMemoryManagerInner::new(&mut subject_bitmap, USABLE_BITMAP_BITS);

        let next_free_standard_index = pmm
            .next_free_frame::<StandardFrame>()
            .map(FrameAddress::index);
        assert_eq!(pmm.bitmap, next_free_standard_bitmap);
        assert_eq!(next_free_standard_index, Some(511));

        let next_free_large_index = pmm.next_free_frame::<LargeFrame>().map(FrameAddress::index);
        assert_eq!(pmm.bitmap, next_free_large_bitmap);
        assert_eq!(next_free_large_index, Some(512));

        println!("{:?}", pmm.bitmap);

        assert_eq!(pmm.next_free_frame::<StandardFrame>(), None);
    }
}
