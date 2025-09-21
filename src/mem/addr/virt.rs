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
            Err(NonCanonicalError::Address(address))
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

    pub fn add_offset(self, offset: usize) -> Result<Self, NonCanonicalError> {
        self.0
            .checked_add(offset)
            .ok_or(NonCanonicalError::PositiveOffset {
                base: self.0,
                offset,
            })
            .and_then(Self::new)
    }

    pub fn sub_offset(self, offset: usize) -> Result<Self, NonCanonicalError> {
        self.0
            .checked_sub(offset)
            .ok_or(NonCanonicalError::NegativeOffset {
                base: self.0,
                offset,
            })
            .and_then(Self::new)
    }

    pub fn min_align(self) -> u32 {
        self.0.trailing_zeros()
    }
}

impl<P: PageAddress> const From<P> for VirtualAddress {
    fn from(value: P) -> Self {
        // Safety: Canonicality of `PageAddress` is a super-set of `VirtualAddress`.
        unsafe { VirtualAddress::new_unchecked(value.into()) }
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

impl core::fmt::LowerHex for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Virtual({:#x})", self.0))
        } else {
            f.write_fmt(format_args!("Virtual({:x})", self.0))
        }
    }
}

impl core::fmt::UpperHex for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Virtual({:#X})", self.0))
        } else {
            f.write_fmt(format_args!("Virtual({:X})", self.0))
        }
    }
}

impl core::fmt::Binary for VirtualAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Virtual({:#b})", self.0))
        } else {
            f.write_fmt(format_args!("Virtual({:b})", self.0))
        }
    }
}

/// # Remarks
///
/// - This trait does not also require e.g. `Into<*mut T>` because that would
///   require indirectly fabricating provenance.
pub trait PageAddress:
    core::fmt::Debug
    + core::fmt::LowerHex
    + core::fmt::UpperHex
    + core::fmt::Binary
    + Clone
    + Copy
    + PartialEq
    + Eq
    + PartialOrd
    + Ord
    + Step
    + const Into<usize>
    + const Into<VirtualAddress>
    + TryFrom<VirtualAddress>
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
            Err(NonCanonicalError::Address(address))
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
            .ok_or(NonCanonicalError::Index {
                align: 1usize << Self::Frame::INDEX_BIT_SHIFT.get(),
                index,
            })?;

        Self::new(address)
    }

    /// Gets the index of the page this address points to.
    fn index(self) -> usize {
        Into::<usize>::into(self) << Self::Frame::INDEX_BIT_SHIFT.get()
    }
}

macro_rules! page_address_impl {
    { $($impl_ty:ty) + } => { $(
        impl core::fmt::LowerHex for $impl_ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if f.alternate() {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:#x})"), self.0))
                } else {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:x})"), self.0))
                }
            }
        }

        impl core::fmt::UpperHex for $impl_ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if f.alternate() {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:#X})"), self.0))
                } else {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:X})"), self.0))
                }
            }
        }

        impl core::fmt::Binary for $impl_ty {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                if f.alternate() {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:#b})"), self.0))
                } else {
                    f.write_fmt(format_args!(concat!(stringify!($impl_ty), "({:b})"), self.0))
                }
            }
        }

        impl Step for $impl_ty {
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

        impl const From<$impl_ty> for usize {
            fn from(value: $impl_ty) -> Self {
                value.0
            }
        }

        impl const TryFrom<$impl_ty> for NonZero<usize> {
            type Error = ZeroAddressError;

            fn try_from(value: $impl_ty) -> Result<Self, Self::Error> {
                NonZero::new(usize::from(value)).ok_or(ZeroAddressError)
            }
        }

        impl TryFrom<VirtualAddress> for $impl_ty {
            type Error = NonCanonicalError;

            fn try_from(address: VirtualAddress) -> Result<Self, Self::Error> {
                (address.min_align()
                    >= <Self as PageAddress>::Frame::SIZE_IN_BYTES
                        .get()
                        .trailing_zeros())
                .then_some({
                    // Safety: Canonicality is checked
                    unsafe { Self::new_unchecked(usize::from(address)) }
                })
                .ok_or(NonCanonicalError::Address(usize::from(address)))
            }
        }
    )* };
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

page_address_impl! {
    StandardPage LargePage HugePage
}
