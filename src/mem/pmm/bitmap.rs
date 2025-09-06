use crate::{
    mem::pmm::{FrameError, PageSize, PhysicalMemoryManagerKind},
    util::sync::Mutex,
};
use libsys::{
    address::{Address, Frame},
    constants::{large_page_bits, page_bits},
};

type SegmentInner = usize;

#[repr(transparent)]
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Segment(SegmentInner);

impl Segment {
    pub const FULL: Self = Self(SegmentInner::MAX);
    pub const EMPTY: Self = Self(SegmentInner::MIN);
    pub const BITS: u32 = SegmentInner::BITS;
    pub const INDEX_BITS_SHIFT: u32 = Self::BITS.trailing_zeros();
    pub const INDEX_BITS_MASK: usize = (1usize << Self::INDEX_BITS_SHIFT) - 1;

    #[cfg(test)]
    pub const fn new(bits: usize) -> Self {
        Self(bits)
    }

    pub fn is_empty(self) -> bool {
        self == Self::EMPTY
    }

    pub fn is_full(self) -> bool {
        self == Self::FULL
    }

    pub fn get_bit(self, index: usize) -> bool {
        debug_assert!(index < usize::try_from(Self::BITS).unwrap());

        (self.0 & (1 << index)) > 0
    }

    pub fn set_bit(&mut self, index: usize) {
        debug_assert!(index < usize::try_from(Self::BITS).unwrap());

        self.0 |= 1 << index;
    }

    pub fn unset_bit(&mut self, index: usize) {
        debug_assert!(index < usize::try_from(Self::BITS).unwrap());

        self.0 &= !(1 << index);
    }

    pub fn set_empty(&mut self) {
        debug_assert!(self.is_full());

        *self = Self::EMPTY;
    }

    pub fn set_full(&mut self) {
        debug_assert!(self.is_empty());

        *self = Self::FULL;
    }

    pub fn next_free(&mut self) -> Option<u32> {
        if self.is_full() {
            None
        } else {
            match self.0.trailing_ones() {
                free_bit_index @ 0..Self::BITS => {
                    self.set_bit(free_bit_index as usize);

                    Some(free_bit_index)
                }

                Self::BITS => None,

                // Safety: `SegmentInner::leading_ones()` cannot overflow `SegmentInner::BITS`.
                _ => unsafe { core::hint::unreachable_unchecked() },
            }
        }
    }
}

pub struct BitMap<'a> {
    bitmap: Mutex<&'a mut [Segment]>,
    total_frames: usize,
}

impl PhysicalMemoryManagerKind for BitMap<'_> {
    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn next_free_frame(&self, page_size: PageSize) -> Option<Address<Frame>> {
        match page_size {
            PageSize::Standard => {
                let (segment_index, bit_index) = self.bitmap.with_lock(|bitmap| {
                    bitmap
                        .iter_mut()
                        .enumerate()
                        .find_map(|(segment_index, segment)| {
                            segment
                                .next_free()
                                .map(|bit_index| (segment_index, bit_index))
                        })
                })?;

                let frame_index = (segment_index << Segment::INDEX_BITS_SHIFT)
                    | usize::try_from(bit_index).unwrap();
                let frame = Address::<Frame>::from_index(frame_index).unwrap();

                Some(frame)
            }

            PageSize::Large => {
                const SEGMENTS_PER_LARGE_PAGE: u32 =
                    large_page_bits().get() - Segment::INDEX_BITS_SHIFT - page_bits().get();

                let large_page_index = self.bitmap.with_lock(|bitmap| {
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
                let frame = Address::<Frame>::new(frame_index).unwrap();

                Some(frame)
            }

            PageSize::Huge => todo!(),
        }
    }

    fn lock_frame(&self, address: Address<Frame>) -> Result<(), FrameError> {
        let index = address.index();
        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        self.bitmap.with_lock(|bitmap| {
            let segment = bitmap
                .get_mut(segment_index)
                .ok_or(FrameError::OutOfBounds)?;
            segment.set_bit(bit_index);

            Ok(())
        })
    }

    fn free_frame(&self, address: Address<Frame>) -> Result<(), FrameError> {
        let index = address.index();
        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        self.bitmap.with_lock(|bitmap| {
            let segment = bitmap
                .get_mut(segment_index)
                .ok_or(FrameError::OutOfBounds)?;
            segment.unset_bit(bit_index);

            Ok(())
        })
    }

    fn is_locked(&self, address: Address<Frame>) -> Result<bool, FrameError> {
        let index = address.index();
        let segment_index = index.unbounded_shr(Segment::INDEX_BITS_SHIFT);
        let bit_index = index & Segment::INDEX_BITS_MASK;

        self.bitmap.with_lock(|bitmap| {
            let segment = bitmap
                .get_mut(segment_index)
                .ok_or(FrameError::OutOfBounds)?;

            Ok(segment.get_bit(bit_index))
        })
    }
}

// use crate::{
//     mem::HigherHalfDirectMap,
//     util::sync::{Once, RwLock},
// };
// use bitmap::{BitMap, BitMapError};
// use core::{num::NonZero, ptr::NonNull};
// use libsys::{
//     address::{Address, Frame},
//     constants::{page_bits, page_mask, page_size},
//     math::align_up_div,
// };

// #[derive(Debug, Error)]
// pub enum NextFrameError {
//     #[error("the physical memory manager is out of free frames")]
//     NoneFree,
// }

// #[derive(Debug, Error)]
// pub enum FrameError {
//     #[error("attempted to index out of bounds")]
//     OutOfBounds,
// }

// impl From<BitMapError> for FrameError {
//     fn from(error: BitMapError) -> Self {
//         match error {
//             BitMapError::OutOfBounds => Self::OutOfBounds,
//         }
//     }
// }

// pub struct PhysicalMemoryManager<'a> {
//     bitmap: RwLock<BitMap<'a>>,
//     total_frames: usize,
// }

// static PHYSICAL_MEMORY_MANAGER: Once<PhysicalMemoryManager<'static>> =
// Once::new();

// // Safety: `PhysicalMemoryManager` uses interrupt-safe synchronization.
// unsafe impl Send for PhysicalMemoryManager<'_> {}

// impl<'a: 'static> PhysicalMemoryManager<'a> {
//     /// Initializes the static physical memory manager with the provided
//     /// bootloader memory map request.
//     pub fn init(memory_map_request: &limine::request::MemoryMapRequest) {
//         PHYSICAL_MEMORY_MANAGER.call_once(|| {
//             trace!("Beginning Physical Memory Manager initialization...");

//             let memory_map = memory_map_request
//                 .get_response()
//                 .expect("bootloader did not provide a response to the memory
// map request")                 .entries();

//             let last_entry = memory_map.last().unwrap();
//             // While this is the ""total"" physical memory, it should be
// noted it isn't the             // total *installed* memory. Because of
// hardware addressing, reserved             // regions—and other quirks—this
// number will likely be much larger             // than the actual amount of
// installed physical memory the machine has.             let
// total_physical_memory =                 usize::try_from(last_entry.base +
// last_entry.length).unwrap();

//             let total_frames = align_up_div(total_physical_memory,
// page_bits());             trace!("Total frames: {total_frames}
// ({total_physical_memory:#X} Bytes)");

//             // Aligned frame count to the next multiple of `usize`s bit
// count.             let bitmap_size = align_up_div(
//                 total_frames,
//                 NonZero::new(usize::BITS.trailing_zeros()).unwrap(),
//             );
//             // Total memory the bitmap will consume as a multiple of frame
// size.             let bitmap_size_in_frames =
//                 align_up_div(bitmap_size * core::mem::size_of::<usize>(),
// page_bits());             // Total memory the bitmap will consume as a
// multiple of bytes.             let bitmap_size_in_bytes =
// bitmap_size_in_frames * page_size();

//             // Inlining the format args breaks rustfmt for some reason.
//             #[allow(clippy::uninlined_format_args)]
//             {
//                 trace!(
//                     "Bitmap {{ Size: {:#X}, Area (Frames): {:#X}, Area
// (Bytes): {:#X} }}",                     bitmap_size, bitmap_size_in_frames,
// bitmap_size_in_bytes                 );
//             }

//             // Select a region that will fit the bitmap, aligned to frame
// size.             let bitmap_region = memory_map
//                 .iter()
//                 .filter(|entry| entry.entry_type ==
// limine::memory_map::EntryType::USABLE)                 .map(|entry| {
//                     let entry_start = usize::try_from(entry.base).unwrap();
//                     let entry_end = usize::try_from(entry.base +
// entry.length).unwrap();

//                     entry_start..entry_end
//                 })
//                 .find(|region| region.len() >= bitmap_size_in_bytes)
//                 .map(|region| region.start..(region.start +
// bitmap_size_in_bytes))                 .expect("no memory regions large
// enough for frame bitmap");

//             debug_assert_eq!(bitmap_region.start & page_mask(), 0);
//             debug_assert_eq!(bitmap_region.end & page_mask(), 0);

//             trace!("Frame bitmap region: {bitmap_region:#X?}");

//             let bitmap_address =
// HigherHalfDirectMap::offset(bitmap_region.start);             let bitmap_ptr
// = NonNull::with_exposed_provenance(bitmap_address);             let
// bitmap_ptr = NonNull::slice_from_raw_parts(bitmap_ptr, bitmap_size);

//             trace!("Physical memory manager bitmap creating...");
//             // Safety:
//             // - Pointer is aligned to `usize`.
//             // - Pointer has no contexts aliasing it (guaranteed by
// bootloader memory map to             //   be unused).
//             let mut bitmap = unsafe {
// BitMap::<'static>::new_from_ptr(bitmap_ptr, total_frames) };
// trace!("Physical memory manager bitmap created.");

//             // Ensure the bitmap's frames are reserved.
//             trace!("Locking: {bitmap_region:#X?}");
//             let bitmap_region_start_index = bitmap_region.start /
// page_size();             let bitmap_region_end_index = bitmap_region.end /
// page_size();             bitmap
//                 .set(bitmap_region_start_index..bitmap_region_end_index)
//                 .unwrap();

//             let mut prev_entry_range_end = None;
//             memory_map
//                 .iter()
//                 .map(|entry| {
//                     // Map the entry to a usable range and type

//                     let entry_start = usize::try_from(entry.base).unwrap();
//                     let entry_end = usize::try_from(entry.base +
// entry.length).unwrap();

//                     (entry_start..entry_end, entry.entry_type)
//                 })
//                 .for_each(|(entry_range, entry_ty)| {
//                     // If there's space inbetween entries, we'll lock it to
// ensure it isn't                     // accidentally used.
//                     if let Some(prev_entry_range_end) = prev_entry_range_end
//                         && prev_entry_range_end < entry_range.start
//                     {
//                         trace!(
//                             "Locking (Inbetween): {:#X?}",
//                             prev_entry_range_end..entry_range.start
//                         );
//                         let lock_start_index = prev_entry_range_end /
// page_size();                         let lock_end_index = entry_range.start /
// page_size();
// bitmap.set(lock_start_index..lock_end_index).unwrap();                     }

//                     // Only lock the non-usable entries...
//                     if entry_ty != limine::memory_map::EntryType::USABLE {
//                         trace!("Locking (Used): {entry_range:#X?}");
//                         let lock_start_index = entry_range.start /
// page_size();                         let lock_end_index = entry_range.end /
// page_size();
// bitmap.set(lock_start_index..lock_end_index).unwrap();                     }

//                     prev_entry_range_end = Some(entry_range.end);
//                 });

//             debug!("Physical memory manager initialized.");

//             Self {
//                 bitmap: RwLock::new(bitmap),
//                 total_frames,
//             }
//         });
//     }

//     fn get_static() -> &'static Self {
//         PHYSICAL_MEMORY_MANAGER.get().unwrap()
//     }

//     fn with_bitmap<T>(func: impl FnOnce(&BitMap<'a>) -> T) -> T {
//         Self::get_static().bitmap.with_shared(func)
//     }

//     fn with_bitmap_mut<T>(func: impl FnOnce(&mut BitMap<'a>) -> T) -> T {
//         Self::get_static().bitmap.with_exclusive(func)
//     }

//     pub fn total_frames() -> usize {
//         Self::get_static().total_frames
//     }

//     pub fn total_memory() -> usize {
//         Self::total_frames() * page_size()
//     }

//     pub fn next_free(
//         count: NonZero<usize>,
//         clear_memory: bool,
//     ) -> Result<Address<Frame>, NextFrameError> {
//         Self::with_bitmap_mut(|bitmap| {
//             let free_frame_index =
// bitmap.next_free(count).ok_or(NextFrameError::NoneFree)?;

//             if count.get() == 1 {
//                 trace!("Frame Locked: {free_frame_index:#X}",);
//             } else {
//                 trace!(
//                     "Frames Locked: {:#X?}",
//                     free_frame_index..(free_frame_index + count.get())
//                 );
//             }

//             let free_frame_address = free_frame_index << page_bits().get();
//             let frame = Address::<Frame>::new(free_frame_address).unwrap();

//             if clear_memory {
//                 // Safety: Memory was just allocated, and is not currently
// aliased.                 unsafe {
//                     crate::mem::zero_frame(frame);
//                 }
//             }

//             Ok(frame)
//         })
//     }

//     pub fn lock_frame(address: Address<Frame>) -> Result<(), FrameError> {
//         Self::with_bitmap_mut(|bitmap| {
//             let index = address.index();

//             debug_assert!(bitmap.get(index)?);

//             bitmap.set(index)?;

//             trace!("Frame Locked: {:#X?}", index << page_bits().get());

//             Ok(())
//         })
//     }

//     pub fn free_frame(address: Address<Frame>) -> Result<(), FrameError> {
//         Self::with_bitmap_mut(|bitmap| {
//             let index = address.index();

//             debug_assert!(!(bitmap.get(index)?));

//             bitmap.unset(index)?;

//             trace!("Freed: {:#X?}", index << page_bits().get());

//             Ok(())
//         })
//     }

//     pub fn is_locked(address: Address<Frame>) -> Result<bool, FrameError> {
//         Self::with_bitmap(|bitmap| Ok(bitmap.get(address.index())?))
//     }
// }
