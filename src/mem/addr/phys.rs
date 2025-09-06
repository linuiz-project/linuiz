use crate::mem::{
    addr::NonCanonicalError,
    mapper::paging::{Depth, PageTableInfo},
};
use core::{fmt::Debug, iter::Step, num::NonZero};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    /// Number of bits in a canonical physical address.
    pub const fn canonical_bits() -> NonZero<u32> {
        NonZero::<u32>::new(52).unwrap()
    }

    /// The maximum physical address.
    pub const fn canonical_max() -> NonZero<usize> {
        NonZero::<usize>::new(1 << Self::canonical_bits().get()).unwrap()
    }

    /// Bit-mask of canonical physical bits.
    pub const fn canonical_mask() -> NonZero<usize> {
        NonZero::<usize>::new(Self::canonical_max().get() - 1).unwrap()
    }

    pub const fn check_canonical(address: usize) -> bool {
        (address & !Self::canonical_mask().get()) == 0
    }

    /// Creates a new [`PhysicalAddress`] with the provided address.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `address` contains any non-canonical bits.
    pub const fn new(address: usize) -> Result<Self, NonCanonicalError> {
        if Self::check_canonical(address) {
            Ok(Self(address))
        } else {
            Err(NonCanonicalError)
        }
    }

    /// Creates a new [`PhysicalAddress`] with the provided address, truncating
    /// any non-canonical bits.
    pub const fn new_truncate(address: usize) -> Self {
        Self(address & Self::canonical_mask().get())
    }

    /// Creates a new [`PhysicalAddress`] without any checks.
    ///
    /// # Safety
    ///
    /// - `address` must have only canonical physical address bits set.
    pub const unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl const From<PhysicalAddress> for usize {
    fn from(value: PhysicalAddress) -> Self {
        value.0
    }
}

impl const TryFrom<usize> for PhysicalAddress {
    type Error = NonCanonicalError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

#[rustfmt::skip]
pub const trait FrameAddress:
    Debug + Clone + Copy + PartialEq + Eq + PartialOrd + Ord + const Into<usize> + const Into<PhysicalAddress> + const TryFrom<PhysicalAddress, Error: Debug> 
{

    fn paging_depth() -> Depth;

    /// Bit shift required to offset this frame's indexes.
    fn index_bit_shift() -> NonZero<u32>;

    /// Bit-mask of the lower non-index bits.
    fn non_index_bit_mask() -> usize {
        Self::size_in_bytes() - 1
    }

    /// The size of this frame in bytes.
    fn size_in_bytes() -> usize {
        1 << Self::index_bit_shift().get()
    }

    /// Size of this frame in standard frames.
    fn size_in_frames() -> NonZero<usize> {
        debug_assert!((Self::size_in_bytes() >> StandardFrame::index_bit_shift().get()) > 0);

        // Safety: Value is non-zero.
        unsafe {
            NonZero::new_unchecked(Self::size_in_bytes() >> StandardFrame::index_bit_shift().get())
        }
    }

    fn canonical_mask() -> NonZero<usize> {
        NonZero::<usize>::new(
            PhysicalAddress::canonical_mask().get() & !Self::non_index_bit_mask(),
        )
        .unwrap()
    
    }

    fn check_canonical(address: usize) -> bool  {
        (address  & !Self::canonical_mask().get()) == 0
    }

    /// Creates a new [`FrameAddress`] with the provided address.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `address` contains any non-canonical bits.
    fn new(address: usize) -> Result<Self, NonCanonicalError> {
        if Self::check_canonical(address) {
            // Safety: Canonicality has been checked.
            Ok(unsafe { Self::new_unchecked(address)})
        } else {
            Err(NonCanonicalError)
        }
    }

    /// Creates a new [`FrameAddress`] with the provided address, truncating
    /// any non-canonical bits.
    fn new_truncate(address: usize) -> Self {
        let address = address & Self::canonical_mask().get();
        // Safety: `address` has non-canonical bits removed.
        unsafe {Self::new_unchecked(address)}
    }

    /// Creates a new [`FrameAddress`] without any checks.
    ///
    /// # Safety
    ///
    /// - `address` must be standard-page-aligned.
    /// - `address` must have only canonical physical address bits set.
    unsafe fn new_unchecked(address: usize) -> Self;

    /// Creates a new [`FrameAddress`] with the provided frame index.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `index` would create a non-canonical address.
    fn from_index(index: usize) -> Result<Self, NonCanonicalError>{
     
        let address = index
            .checked_shl(Self::index_bit_shift().get())
            .ok_or(NonCanonicalError)?;

            Self::new(address)
    }

    /// The index (in strides of [`FrameAddress::size_in_frames`]) of the frame.
    fn index(self) -> usize {
        Into::<usize>::into(self) >> Self::index_bit_shift().get()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StandardFrame(usize);

impl const FrameAddress for StandardFrame {
    fn paging_depth() -> Depth {
        Depth::max()
    }

    fn index_bit_shift() -> NonZero<u32> {
        // Safety: Value is non-zero.
        unsafe { NonZero::<u32>::new_unchecked(12) }
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl const From<StandardFrame> for usize {
    fn from(value: StandardFrame) -> Self {
        value.0
    }
}

impl const From<StandardFrame> for PhysicalAddress {
    fn from(value: StandardFrame) -> Self {
        // Safety: Canonicality of `Self` is superset of `PhysicalAddress`.
        unsafe { PhysicalAddress::new_unchecked(value.0) }
    }
}

impl const TryFrom<PhysicalAddress> for StandardFrame {
    type Error = NonCanonicalError;

    fn try_from(value: PhysicalAddress) -> Result<Self, Self::Error> {
        Self::new(usize::from(value))
    }
}

impl Step for StandardFrame {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        Step::steps_between(&start.index(), &end.index())
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_sub(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LargeFrame(usize);

impl const FrameAddress for LargeFrame {
    fn paging_depth() -> Depth {
        Depth::large()
    }

    fn index_bit_shift() -> NonZero<u32> {
        // Safety: Value is non-zero.
        unsafe {
            NonZero::<u32>::new_unchecked(
                StandardFrame::index_bit_shift().get() + PageTableInfo::index_bits().get(),
            )
        }
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl const From<LargeFrame> for usize {
    fn from(value: LargeFrame) -> Self {
        value.0
    }
}

impl const From<LargeFrame> for PhysicalAddress {
    fn from(value: LargeFrame) -> Self {
        // Safety: Canonicality of `Self` is superset of `PhysicalAddress`.
        unsafe { PhysicalAddress::new_unchecked(value.0) }
    }
}

impl const TryFrom<PhysicalAddress> for LargeFrame {
    type Error = NonCanonicalError;

    fn try_from(value: PhysicalAddress) -> Result<Self, Self::Error> {
        Self::new(usize::from(value))
    }
}

impl Step for LargeFrame {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        Step::steps_between(&start.index(), &end.index())
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_sub(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HugeFrame(usize);

impl const FrameAddress for HugeFrame {
    fn paging_depth() -> Depth {
        Depth::huge()
    }

    fn index_bit_shift() -> NonZero<u32> {
        // Safety: Value is non-zero.
        unsafe {
            NonZero::<u32>::new_unchecked(
                LargeFrame::index_bit_shift().get() + PageTableInfo::index_bits().get(),
            )
        }
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl const From<HugeFrame> for usize {
    fn from(value: HugeFrame) -> Self {
        value.0
    }
}

impl const From<HugeFrame> for PhysicalAddress {
    fn from(value: HugeFrame) -> Self {
        // Safety: Canonicality of `Self` is superset of `PhysicalAddress`.
        unsafe { PhysicalAddress::new_unchecked(value.0) }
    }
}

impl const TryFrom<PhysicalAddress> for HugeFrame {
    type Error = NonCanonicalError;

    fn try_from(value: PhysicalAddress) -> Result<Self, Self::Error> {
        Self::new(usize::from(value))
    }
}

impl Step for HugeFrame {
    fn steps_between(start: &Self, end: &Self) -> (usize, Option<usize>) {
        Step::steps_between(&start.index(), &end.index())
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        start
            .index()
            .checked_sub(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}
