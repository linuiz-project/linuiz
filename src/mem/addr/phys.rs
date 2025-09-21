use crate::{
    mem::{
        addr::NonCanonicalError,
        mapper::paging::{Depth, PagingInfo},
    },
    util::sync::Lazy,
};
use core::{iter::Step, num::NonZero};

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    /// Number of bits in a canonical physical address.
    pub fn canonical_bits() -> NonZero<u32> {
        static CANONICAL_BITS: Lazy<NonZero<u32>> = Lazy::new(|| {
            cfg_select! {
                test => {
                    unsafe { NonZero::<u32>::new_unchecked(40) }
                }

                target_arch = "x86_64" => {
                    let canonical_bits = crate::arch::x86_64::cpuid::processor_capacity_info()
                        .map_or_else(
                            || {
                                if crate::arch::x86_64::cpuid::feature_info().is_some_and(|i| i.has_pae()) {
                                    36
                                } else {
                                    32
                                }
                            },
                            |capacity_info| capacity_info.physical_address_bits()
                        );


                    let canonical_bits = u32::from(canonical_bits);
                    // Safety: Value is always non-zero.
                    unsafe { NonZero::<u32>::new_unchecked(canonical_bits) }
                }

                _ => { unimplemented!() }
            }
        });

        *CANONICAL_BITS
    }

    /// The maximum physical address.
    pub fn canonical_max() -> NonZero<usize> {
        let canonical_max = 1usize << Self::canonical_bits().get();
        // Safety: Value is always non-zero.
        unsafe { NonZero::<usize>::new_unchecked(canonical_max) }
    }

    /// Bit-mask of canonical physical bits.
    pub fn canonical_mask() -> NonZero<usize> {
        let canonical_mask = Self::canonical_max().get() - 1;
        // Safety: Value is always non-zero.
        unsafe { NonZero::<usize>::new_unchecked(canonical_mask) }
    }

    pub fn check_canonical(address: usize) -> bool {
        address <= Self::canonical_max().get()
    }

    /// Creates a new [`PhysicalAddress`] with the provided address.
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

    /// Creates a new [`PhysicalAddress`] with the provided address, truncating
    /// any non-canonical bits.
    pub fn new_truncate(address: usize) -> Self {
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

impl const From<PhysicalAddress> for usize {
    fn from(value: PhysicalAddress) -> Self {
        value.0
    }
}

impl TryFrom<usize> for PhysicalAddress {
    type Error = NonCanonicalError;

    fn try_from(value: usize) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl core::fmt::LowerHex for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Physical({:#x})", self.0))
        } else {
            f.write_fmt(format_args!("Physical({:x})", self.0))
        }
    }
}

impl core::fmt::UpperHex for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Physical({:#X})", self.0))
        } else {
            f.write_fmt(format_args!("Physical({:X})", self.0))
        }
    }
}

impl core::fmt::Binary for PhysicalAddress {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if f.alternate() {
            f.write_fmt(format_args!("Physical({:#b})", self.0))
        } else {
            f.write_fmt(format_args!("Physical({:b})", self.0))
        }
    }
}

pub trait FrameAddress:
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
    + TryFrom<PhysicalAddress, Error = NonCanonicalError>
    + const Into<usize>
    + const Into<PhysicalAddress>
{
    const INDEX_BIT_SHIFT: NonZero<u32>;
    const SIZE_IN_BYTES: NonZero<usize> =
        NonZero::new(1usize << Self::INDEX_BIT_SHIFT.get()).unwrap();
    const SIZE_IN_FRAMES: NonZero<usize> =
        NonZero::new(Self::SIZE_IN_BYTES.get() >> StandardFrame::INDEX_BIT_SHIFT.get()).unwrap();
    const NON_INDEX_BIT_MASK: NonZero<usize> = NonZero::new(Self::SIZE_IN_BYTES.get() - 1).unwrap();

    fn paging_depth() -> Depth;

    fn canonical_mask() -> NonZero<usize> {
        let canonical_mask =
            PhysicalAddress::canonical_mask().get() & !Self::NON_INDEX_BIT_MASK.get();
        // Safety: Value is always non-zero.
        unsafe { NonZero::<usize>::new_unchecked(canonical_mask) }
    }

    fn check_canonical(address: usize) -> bool {
        (address & !Self::canonical_mask().get()) == 0
    }

    /// Creates a new [`FrameAddress`] with the provided address.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `address` contains any non-canonical bits.
    fn new(address: usize) -> Result<Self, NonCanonicalError> {
        if Self::check_canonical(address) {
            // Safety: Canonicality has been checked.
            Ok(unsafe { Self::new_unchecked(address) })
        } else {
            Err(NonCanonicalError::Address(address))
        }
    }

    /// Creates a new [`FrameAddress`] with the provided address, truncating
    /// any non-canonical bits.
    fn new_truncate(address: usize) -> Self {
        let address = address & Self::canonical_mask().get();
        // Safety: `address` has non-canonical bits removed.
        unsafe { Self::new_unchecked(address) }
    }

    /// Creates a new [`FrameAddress`] without any checks.
    ///
    /// # Safety
    ///
    /// - `address` must be standard-page-aligned.
    /// - `address` must have only canonical physical address bits set.
    unsafe fn new_unchecked(address: usize) -> Self;

    /// Creates a new [`FrameAddress`] from the provided frame index.
    ///
    /// # Errors
    ///
    /// - [`NonCanonicalError`] if `index` would create a non-canonical address.
    fn from_index(index: usize) -> Result<Self, NonCanonicalError> {
        let address =
            index
                .checked_shl(Self::INDEX_BIT_SHIFT.get())
                .ok_or(NonCanonicalError::Index {
                    align: 1usize << Self::INDEX_BIT_SHIFT.get(),
                    index,
                })?;

        Self::new(address)
    }

    /// Creates a new [`FrameAddress`] from the provided frame index, without
    /// checking canonicality.
    unsafe fn from_index_unchecked(index: usize) -> Self {
        // Safety: Caller is required to maintaqin safety invariants.
        unsafe { Self::new_unchecked(index.unchecked_shl(Self::INDEX_BIT_SHIFT.get())) }
    }

    /// The index (indexed strides of [`FrameAddress::SIZE_IN_FRAMES`]) of the
    /// frame.
    fn index(self) -> usize {
        Into::<usize>::into(self) >> Self::INDEX_BIT_SHIFT.get()
    }

    /// The index (indexed strides of 1) of the frame.
    fn standard_index(self) -> usize {
        Into::<usize>::into(self) >> StandardFrame::INDEX_BIT_SHIFT.get()
    }
}

macro_rules! frame_address_impl {
    { $($impl_ty:ty)+ } => { $(
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

        impl const From<$impl_ty> for usize {
            fn from(value: $impl_ty) -> Self {
                value.0
            }
        }

        impl const From<$impl_ty> for PhysicalAddress {
            fn from(value: $impl_ty) -> Self {
                // Safety: Canonicality of `Self` is superset of `PhysicalAddress`.
                unsafe { PhysicalAddress::new_unchecked(value.0) }
            }
        }

        impl TryFrom<PhysicalAddress> for $impl_ty {
            type Error = NonCanonicalError;

            fn try_from(value: PhysicalAddress) -> Result<Self, Self::Error> {
                Self::new(usize::from(value))
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
                    .checked_sub(count)
                    .and_then(|next_index| Self::from_index(next_index).ok())
            }
        }
    )+ };
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct StandardFrame(usize);

impl FrameAddress for StandardFrame {
    const INDEX_BIT_SHIFT: NonZero<u32> = NonZero::new(12).unwrap();

    fn paging_depth() -> Depth {
        Depth::max()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct LargeFrame(usize);

impl FrameAddress for LargeFrame {
    const INDEX_BIT_SHIFT: NonZero<u32> = StandardFrame::INDEX_BIT_SHIFT
        .checked_add(PagingInfo::TABLE_INDEX_BITS.get())
        .unwrap();

    fn paging_depth() -> Depth {
        Depth::large()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct HugeFrame(usize);

impl FrameAddress for HugeFrame {
    const INDEX_BIT_SHIFT: NonZero<u32> = LargeFrame::INDEX_BIT_SHIFT
        .checked_add(PagingInfo::TABLE_INDEX_BITS.get())
        .unwrap();

    fn paging_depth() -> Depth {
        Depth::huge()
    }

    unsafe fn new_unchecked(address: usize) -> Self {
        Self(address)
    }
}

frame_address_impl! {
    StandardFrame LargeFrame HugeFrame
}
