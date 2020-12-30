#![no_std]
#![allow(dead_code)]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub use uefi::{
    table::{boot::MemoryDescriptor, Runtime, SystemTable},
    Status,
};

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

// this is used to construct a FramebufferDriver from the kernel
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct FramebufferPointer {
    pub pointer: *mut u8,
    pub size: Size,
}

#[repr(C)]
pub struct BootInfo<'info> {
    mmap_iter: &'info dyn ExactSizeIterator<Item = &'info MemoryDescriptor>,
    runtime_table: SystemTable<Runtime>,
    framebuffer: Option<FramebufferPointer>,
}

pub type KernelMain = extern "win64" fn(crate::BootInfo) -> Status;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "win64" fn __impl_kernel_main(boot_info: $crate::BootInfo) -> $uefi::Status {
            let function: $crate::KernelMain = $path;
            function(boot_info)
        }
    };
}
