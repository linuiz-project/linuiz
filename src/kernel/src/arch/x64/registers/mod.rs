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
            pub unsafe fn write(value: u64) {
                core::arch::asm!(concat!("mov ", stringify!($register_ident), ", {}"), in(reg) value, options(nomem, nostack));
            }

            #[inline]
            pub fn read() -> u64 {
                let value: u64;

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

#[derive(Debug, Clone)]
pub struct Stateful {
    pub cs: u64,
    pub ss: u64,
    pub flags: crate::arch::x64::registers::RFlags,
}

impl Stateful {
    pub fn kernel_state(flags: crate::arch::x64::registers::RFlags) -> Self {
        Self {
            cs: crate::arch::x64::structures::gdt::kernel_code_selector().0 as u64,
            ss: crate::arch::x64::structures::gdt::kernel_data_selector().0 as u64,
            flags,
        }
    }

    pub fn user_state(flags: crate::arch::x64::registers::RFlags) -> Self {
        Self {
            cs: crate::arch::x64::structures::gdt::user_code_selector().0 as u64,
            ss: crate::arch::x64::structures::gdt::user_data_selector().0 as u64,
            flags,
        }
    }
}

#[repr(C)]
#[derive(Debug, Default, Clone)]
pub struct GeneralPurpose {
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
}

impl GeneralPurpose {
    pub const fn empty() -> Self {
        Self {
            rax: 0,
            rbx: 0,
            rcx: 0,
            rdx: 0,
            rsi: 0,
            rdi: 0,
            rbp: 0,
            r8: 0,
            r9: 0,
            r10: 0,
            r11: 0,
            r12: 0,
            r13: 0,
            r14: 0,
            r15: 0,
        }
    }
}

#[repr(C, packed)]
#[derive(Debug, Default, Clone)]
pub struct PreservedRegistersSysv64 {
    r15: u64,
    r14: u64,
    r13: u64,
    r12: u64,
    rbp: u64,
    rbx: u64,
    rfl: u64,
    rsp: u64,
}
