use crate::mem::{
    addr::{NonCanonicalError, phys::FrameAddress},
    get_paging_depth,
    mapper::paging::{Depth, PagingInfo},
};
use core::{fmt::Debug, iter::Step, num::NonZero, ptr::NonNull};

#[derive(Debug, Error, Clone, Copy)]
#[error("tried to convert a null virtual address to a non-null pointer")]
pub struct ZeroAddressError;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub fn canonical_bits() -> NonZero<u32> {
        use crate::mem::addr::phys::StandardFrame;

        let table_indexes_shift = PagingInfo::TABLE_INDEX_BITS.get() * get_paging_depth().get();
        let total_shift = table_indexes_shift + StandardFrame::INDEX_BIT_SHIFT.get();

        debug_assert!(total_shift > 0);

        // Safety: `total_shift` is always non-zero.
        unsafe { NonZero::<u32>::new_unchecked(total_shift) }
    }

    pub fn check_canonical(address: usize) -> bool {
        #[allow(clippy::as_conversions)]
        let canonical_bits = Self::canonical_bits().get() as usize;
        let sign_extension_check_shift = canonical_bits - 1;
        matches!(address >> sign_extension_check_shift, 0 | 0x1FFFF)
    }

    /// Creates a new [`VirtualAddress`] with the provided address.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `address` contains any non-canonical bits.
    pub fn new(address: usize) -> Result<Self, NonCanonicalError> {
        if Self::check_canonical(address) {
            Ok(Self(address))
        } else {
            Err(NonCanonicalError)
        }
    }

    /// Creates a new [`VirtualAddress`] with the provided address, truncating
    /// any non-canonical bits.
    pub fn new_truncate(address: usize) -> Self {
        let sign_extension_shift = usize::BITS
            .checked_sub(Self::canonical_bits().get())
            .unwrap();

        #[allow(
            clippy::as_conversions,
            clippy::cast_possible_wrap,
            clippy::cast_sign_loss
        )]
        Self(
            (address.unbounded_shl(sign_extension_shift) as isize)
                .unbounded_shr(sign_extension_shift) as usize,
        )
    }

    /// # Safety
    ///
    /// - `address` must have only canonical virtual address bits set.
    pub const unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }

    pub const fn is_null(self) -> bool {
        self.0 == 0
    }
}

impl<P: PageAddress> const From<P> for VirtualAddress {
    fn from(value: P) -> Self {
        let address: usize = value.into();
        // Safety: Canonicality of `LargePage` is a super-set of `VirtualAddress`.
        unsafe { VirtualAddress::new_unchecked(address) }
    }
}

impl TryFrom<usize> for VirtualAddress {
    type Error = NonCanonicalError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        VirtualAddress::new(value)
    }
}

impl const From<VirtualAddress> for usize {
    fn from(value: VirtualAddress) -> Self {
        value.0
    }
}

impl const TryFrom<VirtualAddress> for NonZero<usize> {
    type Error = ZeroAddressError;

    fn try_from(value: VirtualAddress) -> Result<Self, Self::Error> {
        NonZero::new(usize::from(value)).ok_or(ZeroAddressError)
    }
}

impl<T> TryFrom<*mut T> for VirtualAddress {
    type Error = NonCanonicalError;

    fn try_from(value: *mut T) -> Result<Self, Self::Error> {
        Self::new(value.addr())
    }
}

impl<T> TryFrom<NonNull<T>> for VirtualAddress {
    type Error = NonCanonicalError;

    fn try_from(value: NonNull<T>) -> Result<Self, Self::Error> {
        Self::new(value.addr().get())
    }
}

/// # Remarks
///
/// - This trait does not also require e.g. `Into<*mut T>` because that would
///   require indirectly fabricating provenance.
pub trait PageAddress:
    Debug
    + Clone
    + Copy
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + const Into<usize>
    + Into<VirtualAddress>
    + Step
{
    type Frame: FrameAddress;

    fn paging_depth() -> Depth;

    fn check_canonical(address: usize) -> bool {
        ((address & Self::Frame::NON_INDEX_BIT_MASK.get()) == 0)
            && VirtualAddress::check_canonical(address)
    }

    /// Creates a new [`PageAddress`] with the provided address.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `address` contains any non-canonical bits.
    fn new(address: usize) -> Result<Self, NonCanonicalError> {
        if Self::check_canonical(address) {
            // Safety: `address` is checked to be canonical.
            Ok(unsafe { Self::new_unchecked(address) })
        } else {
            Err(NonCanonicalError)
        }
    }

    /// Creates a new [`PageAddress`] with the provided address, truncating any
    /// non-canonical bits.
    fn new_truncate(address: usize) -> Self {
        let address = usize::from(VirtualAddress::new_truncate(address))
            & !Self::Frame::NON_INDEX_BIT_MASK.get();
        // Safety: `address` has non-canonical bits removed.
        unsafe { Self::new_unchecked(address) }
    }

    /// # Safety
    ///
    /// - `address` must be page-aligned.
    /// - `address` must have only canonical virtual address bits set.
    unsafe fn new_unchecked(address: usize) -> Self;

    /// Creates a new [`PageAddress`] with the provided frame index.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `index` would create a non-canonical address.
    fn from_index(index: usize) -> Result<Self, NonCanonicalError> {
        let address = index
            .checked_shl(Self::Frame::INDEX_BIT_SHIFT.get())
            .ok_or(NonCanonicalError)?;

        Self::new(address)
    }

    /// Gets the index of the page this address points to.
    fn index(self) -> usize {
        Into::<usize>::into(self) << Self::Frame::INDEX_BIT_SHIFT.get()
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StandardPage(usize);

impl PageAddress for StandardPage {
    // Direct module addressing is used to ensure only `Self::Frame` is referenced.
    type Frame = crate::mem::addr::phys::StandardFrame;

    fn paging_depth() -> Depth {
        Depth::max()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl Step for StandardPage {
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
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}

impl const From<StandardPage> for usize {
    fn from(value: StandardPage) -> Self {
        value.0
    }
}

impl const TryFrom<StandardPage> for NonZero<usize> {
    type Error = ZeroAddressError;

    fn try_from(value: StandardPage) -> Result<Self, Self::Error> {
        NonZero::new(usize::from(value)).ok_or(ZeroAddressError)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LargePage(usize);

impl PageAddress for LargePage {
    // Direct module addressing is used to ensure only `Self::Frame` is referenced.
    type Frame = crate::mem::addr::phys::LargeFrame;

    fn paging_depth() -> Depth {
        Depth::large()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl Step for LargePage {
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
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}

impl const From<LargePage> for usize {
    fn from(value: LargePage) -> Self {
        value.0
    }
}

impl const TryFrom<LargePage> for NonZero<usize> {
    type Error = ZeroAddressError;

    fn try_from(value: LargePage) -> Result<Self, Self::Error> {
        NonZero::new(usize::from(value)).ok_or(ZeroAddressError)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HugePage(usize);

impl PageAddress for HugePage {
    // Direct module addressing is used to ensure only `Self::Frame` is referenced.
    type Frame = crate::mem::addr::phys::HugeFrame;

    fn paging_depth() -> Depth {
        Depth::huge()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

impl Step for HugePage {
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
            .checked_add(count)
            .and_then(|next_index| Self::from_index(next_index).ok())
    }
}

impl const From<HugePage> for usize {
    fn from(value: HugePage) -> Self {
        value.0
    }
}

impl const TryFrom<HugePage> for NonZero<usize> {
    type Error = ZeroAddressError;

    fn try_from(value: HugePage) -> Result<Self, Self::Error> {
        NonZero::new(usize::from(value)).ok_or(ZeroAddressError)
    }
}
