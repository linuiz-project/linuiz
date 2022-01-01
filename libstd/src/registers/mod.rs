mod cr2;
mod cr3;
mod flags;
mod msr;

pub use cr2::*;
pub use cr3::*;
pub use flags::*;
pub use msr::*;

macro_rules! basic_register_raw {
    ($register_ident:ident) => {
        pub struct $register_ident;

        impl $register_ident {
            #[inline(always)]
            pub unsafe fn write(value: usize) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) value, options(nomem, nostack));
            }

            #[inline(always)]
            pub fn read() -> usize {
                let value: usize;
                unsafe {
                    core::arch::asm!(concat!("mov {}, ", stringify!($register_ident)), out(reg) value, options(nomem, nostack));
                    value as usize
                }
            }
        }
    }
}

macro_rules! basic_register_ptr {
    ($register_ident:ident) => {
        pub struct $register_ident;

        impl $register_ident {
            #[inline(always)]
            pub unsafe fn write(addr: $crate::Address<$crate::addr_ty::Virtual>) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) addr.as_usize(), options(nomem, nostack));
            }

            #[inline(always)]
            pub fn read() -> $crate::Address<$crate::addr_ty::Virtual> {
                let addr: u64;
                unsafe {
                    core::arch::asm!(concat!("mov {}, ", stringify!($register_ident)), out(reg) addr, options(nomem, nostack));
                    $crate::Address::new_unsafe(addr as usize)
                }
            }
        }
    }
}

pub mod debug {
    basic_register_raw! {DR0}
    basic_register_raw! {DR1}
    basic_register_raw! {DR2}
    basic_register_raw! {DR3}
    basic_register_raw! {DR4}
    basic_register_raw! {DR5}
    basic_register_raw! {DR6}
    basic_register_raw! {DR7}
}

pub mod stack {
    basic_register_ptr! {RSP}
    basic_register_ptr! {RBP}
}
