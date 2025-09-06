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
                    #[allow(clippy::as_conversions)]
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
