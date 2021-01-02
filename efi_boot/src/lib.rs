#![no_std]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub use uefi::{
    table::{boot::{MemoryDescriptor, MemoryType}, Runtime, SystemTable},
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
    memory_map: &'info [MemoryDescriptor],
    runtime_table: SystemTable<Runtime>,
    framebuffer: Option<FramebufferPointer>,
}

impl<'info> BootInfo<'info> {
    pub fn new(
        memory_map: &'info [MemoryDescriptor],
        runtime_table: SystemTable<Runtime>,
        framebuffer: Option<FramebufferPointer>,
    ) -> Self {
        Self {
            memory_map,
            runtime_table,
            framebuffer,
        }
    }

    pub fn memory_map(&self) -> &[MemoryDescriptor] {
        self.memory_map
    }

    pub fn runtime_table(&self) -> &SystemTable<Runtime> {
        &self.runtime_table
    }

    pub fn framebuffer_pointer(&self) -> &Option<FramebufferPointer> {
        &self.framebuffer
    }
}

pub type KernelMain = extern "win64" fn(crate::BootInfo) -> Status;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "win64" fn __impl_kernel_main(boot_info: $crate::BootInfo) -> $crate::Status {
            let function: $crate::KernelMain = $path;
            function(boot_info)
        }
    };
}
