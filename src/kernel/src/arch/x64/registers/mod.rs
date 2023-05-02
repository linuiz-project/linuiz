#![allow(dead_code, clippy::upper_case_acronyms)]

mod rflags;

pub use rflags::*;
pub mod control;
pub mod msr {
    pub use msr::*;
}

macro_rules! basic_raw_register {
    ($register_ident:ident) => {
        pub struct $register_ident;

        impl $register_ident {
            /// ### Safety
            ///
            /// Writing directly to a register circumvents the compiler. It is the job of the developer
            /// to ensure that this does not cause undefined behaviour.
            #[inline]
            pub unsafe fn write(value: libsys::ureg) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) value, options(nomem, nostack));
            }

            #[inline]
            pub fn read() -> libsys::ureg {
                let value: libsys::ureg;

                // Safety: Reading a value out of a register does not cause undefined behaviour.
                unsafe {
                    core::arch::asm!(concat!("mov {}, ", stringify!($register_ident)), out(reg) value, options(nomem, nostack));
                }

                value
            }
        }
    }
}

macro_rules! basic_ptr_register {
    ($register_ident:ident) => {
        pub struct $register_ident;

        impl $register_ident {
            /// ### Safety
            ///
            /// Writing directly to a register circumvents the compiler. It is the job of the developer
            /// to ensure that this does not cause undefined behaviour.
            #[inline]
            pub unsafe fn write(ptr: *const ()) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) ptr, options(nomem, nostack, preserves_flags));
            }

            #[inline]
            pub fn read() -> *const () {
                let ptr: *const ();

                // Safety: We are only reading values here.
                unsafe {
                    core::arch::asm!(concat!("mov {}, ", stringify!($register_ident)), out(reg) ptr, options(nomem, nostack, preserves_flags));

                    ptr
                }
            }
        }
    }
}

pub mod debug {
    basic_raw_register! {DR0}
    basic_raw_register! {DR1}
    basic_raw_register! {DR2}
    basic_raw_register! {DR3}
    basic_raw_register! {DR4}
    basic_raw_register! {DR5}
    basic_raw_register! {DR6}
    basic_raw_register! {DR7}
}

pub mod stack {
    basic_ptr_register! {RBP}
    basic_ptr_register! {RSP}
}
