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
    framebuffer: *mut u8,
    size: Size,
}

impl Framebuffer {
    pub fn new(framebuffer: *mut u8, size: Size) -> Self {
        Self { framebuffer, size }
    }
}

pub type KernelMain = extern "C" fn(*const crate::Framebuffer) -> i32;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "C" fn __impl_kernel_main(framebuffer: *const $crate::Framebuffer) -> i32 {
            let function: $crate::KernelMain = $path;
            function(framebuffer)
        }
    };
}
