#![no_std]
#![feature(asm_sym, asm_const, naked_functions, sync_unsafe_cell, exclusive_range_pattern, allocator_api, if_let_guard)]
#![cfg_attr(target_arch = "x86_64", feature(abi_x86_interrupt))]

#[macro_use]
extern crate log;
extern crate alloc;

pub mod interrupts;
pub mod memory;
#[cfg(target_arch = "riscv64")]
pub mod rv64;
#[cfg(target_arch = "x86_64")]
pub mod x64;

pub mod reexport {
    #[cfg(target_arch = "x86_64")]
    pub mod x86_64 {
        pub use x86_64::{PhysAddr, VirtAddr};
    }
}
