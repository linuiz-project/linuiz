#![allow(dead_code, clippy::upper_case_acronyms)]

mod rflags;
pub use rflags::*;

pub mod control;
pub mod model_specific;

pub struct RSP;

impl RSP {
    /// Writes the raw `value` to the stack pointer register.
    ///
    /// # Safety
    ///
    /// Writing directly to a register circumvents the compiler. It is the job of the developer
    /// to ensure that this does not cause undefined behaviour.
    #[inline(always)]
    pub unsafe fn write(value: *const u8) {
        // Safety: Caller is required to ensure no undefined behaviour occurs.
        #[allow(clippy::pointers_in_nomem_asm_block)]
        unsafe {
            core::arch::asm!(
                "mov rsp, {}",
                in(reg) value,
                options(nomem, nostack, preserves_flags)
            );
        }
    }

    // Reads the raw value from the register.
    #[inline(always)]
    pub fn read() -> *const u8 {
        let value: usize;

        // Safety: Reading a value out of a register does not cause undefined behaviour.
        unsafe {
            core::arch::asm!(
                "mov {}, rsp",
                out(reg) value,
                options(nomem, nostack, preserves_flags)
            );
        }

        core::ptr::with_exposed_provenance(value)
    }
}

macro_rules! int_register {
    ($register_ident:ident) => {
        pub struct $register_ident;

        impl $register_ident {
            /// Writes the raw `value` to the register.
            ///
            /// # Safety
            ///
            /// Writing directly to a register circumvents the compiler. It is the job of the developer
            /// to ensure that this does not cause undefined behaviour.
            #[inline(always)]
            pub unsafe fn write(value: u64) {
                // Safety: Caller is required to ensure no undefined behaviour occurs.
                unsafe {
                    core::arch::asm!(
                        concat!("mov ", stringify!($register_ident), ", {}"),
                        in(reg) value,
                        options(nomem, nostack, preserves_flags)
                    );
                }
            }

            // Reads the raw value from the register.
            #[inline(always)]
            pub fn read() -> u64 {
                let value: u64;

                // Safety: Reading a value out of a register does not cause undefined behaviour.
                unsafe {
                    core::arch::asm!(
                        concat!("mov {}, ", stringify!($register_ident)),
                        out(reg) value,
                        options(nomem, nostack, preserves_flags));
                }

                value
            }
        }
    }
}

int_register! {DR0}
int_register! {DR1}
int_register! {DR2}
int_register! {DR3}
int_register! {DR4}
int_register! {DR5}
int_register! {DR6}
int_register! {DR7}
