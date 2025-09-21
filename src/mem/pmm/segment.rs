use crate::mem::addr::phys::{FrameAddress, HugeFrame, LargeFrame, StandardFrame};
use core::num::NonZero;

pub type SegmentRepr = u16;

#[allow(clippy::as_conversions)]
pub const SEGMENT_BITS_USIZE: NonZero<usize> = NonZero::new(Segment::BITS.get() as usize).unwrap();

#[repr(transparent)]
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub struct Segment(SegmentRepr);

impl Segment {
    pub const FULL: Self = Self(SegmentRepr::MAX);
    pub const EMPTY: Self = Self(SegmentRepr::MIN);
    pub const BITS: NonZero<u32> = NonZero::new(SegmentRepr::BITS).unwrap();
    pub const INDEX_BITS_SHIFT: NonZero<u32> = NonZero::new(Self::BITS.trailing_zeros()).unwrap();
    pub const INDEX_BITS_MASK: NonZero<usize> =
        NonZero::new((1usize << Self::INDEX_BITS_SHIFT.get()) - 1).unwrap();
    pub const PER_LARGE_PAGE: NonZero<usize> = NonZero::new(
        1usize
            << (LargeFrame::INDEX_BIT_SHIFT.get()
                - Segment::INDEX_BITS_SHIFT.get()
                - StandardFrame::INDEX_BIT_SHIFT.get()),
    )
    .unwrap();
    pub const PER_HUGE_PAGE: NonZero<usize> = NonZero::new(
        1usize
            << (HugeFrame::INDEX_BIT_SHIFT.get()
                - Segment::INDEX_BITS_SHIFT.get()
                - StandardFrame::INDEX_BIT_SHIFT.get()),
    )
    .unwrap();

    pub const fn new(bits: SegmentRepr) -> Self {
        Self(bits)
    }

    pub(super) fn inner_mut(&mut self) -> &mut SegmentRepr {
        &mut self.0
    }

    pub fn is_empty(self) -> bool {
        self == Self::EMPTY
    }

    pub fn is_full(self) -> bool {
        self == Self::FULL
    }

    pub fn get_bit(self, bit_index: u32) -> bool {
        (self.0 & (1 << bit_index)) > 0
    }

    pub fn set_bit(&mut self, bit_index: u32) {
        self.0 |= 1 << bit_index;
    }

    pub fn unset_bit(&mut self, bit_index: u32) {
        self.0 &= !(1 << bit_index);
    }

    pub fn next_free(&mut self) -> Option<u32> {
        if self.is_full() {
            None
        } else {
            let free_bit_index = self.0.trailing_ones();
            self.set_bit(free_bit_index);

            Some(free_bit_index)
        }
    }
}

impl core::fmt::Debug for Segment {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        #[allow(clippy::as_conversions)]
        const SEGMENT_INNER_BIT_WIDTH: usize = SegmentRepr::BITS as usize;

        f.debug_tuple("Segment")
            .field(&format_args!(
                "{:0>width$b}",
                self.0,
                width = SEGMENT_INNER_BIT_WIDTH,
            ))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::{Segment, SegmentRepr};

    #[test]
    fn new() {
        const INNER_VALUE: SegmentRepr = 0b100000001;
        debug_assert_eq!(Segment::new(INNER_VALUE), Segment(INNER_VALUE));
    }

    #[test]
    fn is_empty() {
        debug_assert!(Segment::new(SegmentRepr::MIN).is_empty());
    }

    #[test]
    fn is_full() {
        debug_assert!(Segment::new(SegmentRepr::MAX).is_full());
    }

    #[test]
    fn get_bit() {
        const SEGMENT: Segment = Segment::new(0b111000);
        debug_assert!(!SEGMENT.get_bit(0));
        debug_assert!(!SEGMENT.get_bit(1));
        debug_assert!(!SEGMENT.get_bit(2));
        debug_assert!(SEGMENT.get_bit(3));
        debug_assert!(SEGMENT.get_bit(4));
        debug_assert!(SEGMENT.get_bit(5));
    }
}
