use crate::{memory::volatile::VolatileCell, ReadWrite};

#[macro_export]
macro_rules! bitslice_primitive {
    ($prim_ty:ty) => {
        impl BitSlicePrimitive for $prim_ty {}

        impl<'slice> BitSlice<'slice, $prim_ty> {
            const BIT_WIDTH: usize = core::mem::size_of::<$prim_ty>() * 8;
            const BIT_MASK: usize = Self::BIT_WIDTH - 1;

            pub fn from_slice(slice: &'slice mut [$prim_ty], len: usize) -> Self {
                Self { slice, len }
            }

            pub fn get_bit(&self, index: usize) -> bool {
                assert!(index < self.len, "Index out of bounds of bit slice.");

                let slice_index = (index & Self::BIT_MASK) >> Self::BIT_MASK;
                let value_index = index & Self::BIT_MASK;

                (self.slice[slice_index] & (1 << value_index)) > 0
            }
        }
    };
}

#[macro_export]
macro_rules! bitslice_volatile {
    ($prim_ty:ty) => {
        impl BitSlicePrimitive for VolatileCell<$prim_ty, ReadWrite> {}

        impl<'slice> BitSlice<'slice, VolatileCell<$prim_ty, ReadWrite>> {
            const BIT_WIDTH: usize = core::mem::size_of::<$prim_ty>() * 8;
            const BIT_MASK: usize = Self::BIT_WIDTH - 1;

            pub fn from_slice(
                slice: &'slice mut [VolatileCell<$prim_ty, ReadWrite>],
                len: usize,
            ) -> Self {
                Self { slice, len }
            }

            pub fn get_bit(&self, index: usize) -> bool {
                assert!(index < self.len, "Index out of bounds of bit slice.");

                let slice_index = (index & Self::BIT_MASK) >> Self::BIT_MASK;
                let value_index = index & Self::BIT_MASK;

                (self.slice[slice_index].read() & (1 << value_index)) > 0
            }
        }

        impl core::fmt::Debug for BitSlice<'_, VolatileCell<$prim_ty, ReadWrite>> {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                let mut debug_list = formatter.debug_list();

                for index in 0..self.len {
                    debug_list.entry(&self.get_bit(index));
                }

                debug_list.finish()
            }
        }
    };
}

pub trait BitSlicePrimitive {}

pub struct BitSlice<'slice, P: BitSlicePrimitive> {
    slice: &'slice mut [P],
    len: usize,
}

bitslice_primitive!(u8);
bitslice_primitive!(u16);
bitslice_primitive!(u32);
bitslice_primitive!(u64);
bitslice_primitive!(usize);

bitslice_volatile!(u8);
bitslice_volatile!(u16);
bitslice_volatile!(u32);
bitslice_volatile!(u64);
bitslice_volatile!(usize);
