#[macro_export]
macro_rules! bitfield_getter_ro {
    ($field:ident, $getter_name:ident, $bit_index:literal) => {
        paste::paste! {
            pub fn [<get_ $getter_name>](&self) -> bool {
                use bit_field::BitField;

                self.$field.get_bit($bit_index)
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            pub fn [<get_ $var_name>](&self) -> $field_ty {
                use bit_field::BitField;

                self.$field.get_bits($bit_range)
            }
        }
    };
}

#[macro_export]
macro_rules! bitfield_getter {
    ($field:ident, $var_name:ident, $bit_index:literal) => {
        paste::paste! {
            $crate::bitfield_getter_ro!($field, $var_name, $bit_index);

            pub fn [<set_ $var_name>](&mut self, value: bool) {
                use bit_field::BitField;

                self.$field.set_bit($bit_index, value);
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            $crate::bitfield_getter_ro!($field, $field_ty, $var_name, $bit_range);

            pub fn [<set_ $var_name>](&mut self, value: $field_ty) {
                use bit_field::BitField;

                self.$field.set_bits($bit_range, value);
            }
        }
    };
}

#[macro_export]
macro_rules! atomic_bitfield_getter_ro {
    ($field:ident, $getter_name:ident, $bit_index:literal) => {
        paste::paste! {
            pub fn [<get_ $getter_name>](&self) -> bool {
                use bit_field::BitField;

                self.$field.load(core::sync::atomic::Ordering::Acquire).get_bit($bit_index)
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            pub fn [<get_ $var_name>](&self) -> $field_ty {
                use bit_field::BitField;

                self.$field.load(core::sync::atomic::Ordering::Acquire).get_bits($bit_range)
            }
        }
    };
}

#[macro_export]
macro_rules! atomic_bitfield_getter {
    ($field:ident, $var_name:ident, $bit_index:literal) => {
        $crate::atomic_bitfield_getter_ro!($field, $var_name, $bit_index);

        paste::paste! {
            pub fn [<set_ $var_name>](&mut self, set: bool) {
                use core::sync::atomic::Ordering;
                use bit_field::BitField;

                self.$field.store(*(self.$field.load(Ordering::Acquire).set_bit($bit_index, set)), Ordering::Release);
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            libkernel::atomic_bitfield_getter_ro!($field, $field_ty, $var_name, $bit_range);

            pub fn [<set_ $var_name>](&mut self, value: $field_ty) {
                use core::sync::atomic::Ordering;
                use bit_field::BitField;

                self.$field.store(*(self.$field.load(Ordering::Acquire).set_bits($bit_range, value)), Ordering::Release);
            }
        }
    };
}

#[macro_export]
macro_rules! volatile_bitfield_getter_ro {
    ($field:ident, $getter_name:ident, $bit_index:literal) => {
        paste::paste! {
            pub fn [<get_ $getter_name>](&self) -> bool {
                use bit_field::BitField;

                self.$field.read().get_bit($bit_index)
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            pub fn [<get_ $var_name>](&self) -> $field_ty {
                use bit_field::BitField;

                self.$field.read().get_bits($bit_range)
            }
        }
    };
}

#[macro_export]
macro_rules! volatile_bitfield_getter {
    ($field:ident, $var_name:ident, $bit_index:literal) => {
        $crate::volatile_bitfield_getter_ro!($field, $var_name, $bit_index);

        paste::paste! {
            pub fn [<set_ $var_name>](&mut self, set: bool) {
                use bit_field::BitField;

                self.$field.write(*self.$field.read().set_bit($bit_index, set));
            }
        }
    };

    ($field:ident, $field_ty:ty, $var_name:ident, $bit_range:expr) => {
        paste::paste! {
            $crate::volatile_bitfield_getter_ro!($field, $field_ty, $var_name, $bit_range);

            pub fn [<set_ $var_name>](&mut self, value: $field_ty) {
                use bit_field::BitField;

                self.$field.write(*self.$field.read().set_bits($bit_range, value));
            }
        }
    };
}
