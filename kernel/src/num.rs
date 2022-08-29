#![allow(dead_code)]

#[macro_export]
macro_rules! little_endian {
    ($name:ident, $num_ty:ty) => {
        #[repr(transparent)]
        #[derive(Clone, Copy)]
        pub struct $name([u8; core::mem::size_of::<$num_ty>()]);

        impl $name {
            /// Creates a safe little endian wrapper around the given native endian integer.
            pub const fn new(value: $num_ty) -> Self {
                Self(value.to_le_bytes())
            }

            /// Gets the contained value by its endianness.
            pub const fn get(self) -> $num_ty {
                <$num_ty>::from_le_bytes(self.0)
            }
        }

        impl Eq for $name {}
        impl PartialEq for $name {
            fn eq(&self, other: &Self) -> bool {
                self.get() == other.get()
            }
        }

        impl core::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.debug_tuple("$name").field(&self.get()).finish()
            }
        }
    };
}

crate::little_endian!(LittleEndianU8, u8);
crate::little_endian!(LittleEndianU16, u16);
crate::little_endian!(LittleEndianU32, u32);
crate::little_endian!(LittleEndianU64, u64);
