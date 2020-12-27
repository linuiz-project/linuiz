#![no_std]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub use uefi::table::{Runtime, SystemTable};

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    pub width: usize,
    pub height: usize,
}

impl Size {
    pub fn new(width: usize, height: usize) -> Self {
        Size { width, height }
    }

    pub fn length(&self) -> usize {
        self.width * self.height
    }
}

// this is used to construct a FramebufferDriver from the kernel
#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Framebuffer {
    pub pointer: *mut u8,
    pub size: Size,
}

impl Framebuffer {
    pub fn new(pointer: *mut u8, size: Size) -> Self {
        Self { pointer, size }
    }
}

pub type KernelMain = extern "win64" fn(Option<crate::Framebuffer>) -> i32;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "win64" fn __impl_kernel_main(framebuffer: Option<$crate::Framebuffer>) -> i32 {
            let function: $crate::KernelMain = $path;
            function(framebuffer)
        }
    };
}
