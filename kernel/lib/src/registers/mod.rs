mod rflags;
mod tsc;

pub mod control;
pub mod msr;
pub use rflags::*;
pub use tsc::*;

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
            pub unsafe fn write(ptr: *const ()) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) ptr, options(nomem, nostack));
            }

            #[inline(always)]
            pub fn read() -> *const () {
                let ptr: *const ();
                unsafe {
                    core::arch::asm!(concat!("mov {}, ", stringify!($register_ident)), out(reg) ptr, options(nomem, nostack));
                    ptr
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
    basic_register_ptr! {RBP}

    pub struct RSP;
    impl RSP {
        #[inline(always)]
        pub unsafe fn write(ptr: *mut ()) {
            core::arch::asm!("mov rsp, {}", in(reg) ptr, options(nomem));
        }

        #[inline(always)]
        pub fn read() -> *const () {
            let ptr: *const ();
            unsafe {
                core::arch::asm!("mov {}, rsp", out(reg) ptr, options(nomem, nostack));
                ptr
            }
        }
    }
}
