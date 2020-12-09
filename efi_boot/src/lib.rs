#![no_std]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub mod drivers;
mod memory;

pub use uefi::table::SystemTable;

pub type KernelMain = fn(crate::drivers::graphics::ProtocolGraphics) -> i32;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub fn __impl_kernel_main(
            protocol_graphics: $crate::drivers::graphics::ProtocolGraphics,
        ) -> i32 {
            let function: $crate::KernelMain = $path;
            function(protocol_graphics)
        }
    };
}
