use crate::mem::pmm::{FrameError, PageSize, PhysicalMemoryManagerKind};
use core::usize;
use libsys::address::{Address, Frame};

type Segment = u8;

const BITMAP_INDEX_SHIFT: u32 = Segment::BITS.trailing_zeros();
const BIT_INDEX_MASK: usize = {
    let shift = usize::BITS - BITMAP_INDEX_SHIFT;
    usize::MAX.unbounded_shl(shift).unbounded_shr(shift)
};

fn level_to_bit_offset(level: u32) -> usize {
    ((1usize << (level + 1)) - 1) & !1
}

pub struct Buddy<'a> {
    bitmap: &'a mut [u8],
    max_level: u32,
    total_frames: usize,
}

impl<'a> Buddy<'a> {
    pub fn new<'b: 'a>(bitmap: &'b mut [Segment], max_level: u32, total_frames: usize) -> Self {
        Self {
            bitmap,
            max_level,
            total_frames,
        }
    }

    fn find_free_bit_by_level(&self, _level: u32) -> Result<Option<usize>, ()> {
        // if level > self.max_level {
        //     return Err(todo!());

        // }

        // let level_bit_width = 1usize << (level + 1);
        // let level_bit_offset = (level_bit_width - 1) & !1;

        // let bitmap_start_index = level_bit_offset.unbounded_shr(BITMAP_INDEX_SHIFT);
        // let segment_start_bit = level_bit_offset & BIT_INDEX_MASK;
        // let mut remaining_bits = level_bit_width;

        //  self.bitmap.iter().enumerate().skip(bitmap_start_index).take_while(|_|
        // remaining_bits > 0).find(|(index, segment)| {     todo!()

        //  });

        todo!()
    }
}

impl PhysicalMemoryManagerKind for Buddy<'_> {
    fn total_frames(&self) -> usize {
        self.total_frames
    }

    fn next_free_frame(&self, _page_size: PageSize) -> Option<Address<Frame>> {
        todo!()
    }

    fn lock_frame(&self, _address: Address<Frame>) -> Result<(), FrameError> {
        todo!()
    }

    fn free_frame(&self, _address: Address<Frame>) -> Result<(), FrameError> {
        todo!()
    }

    fn is_locked(&self, _address: Address<Frame>) -> Result<bool, FrameError> {
        todo!()
    }
}
