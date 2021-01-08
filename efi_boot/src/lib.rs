#![no_std]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub mod elf;
pub mod memory;

pub use uefi::{
    table::{
        boot::{MemoryDescriptor, MemoryType},
        runtime::ResetType,
        Runtime, SystemTable,
    },
    Status,
};

pub const KERNEL_CODE: MemoryType = MemoryType::custom(0xFFFFFF00);
pub const KERNEL_DATA: MemoryType = MemoryType::custom(0xFFFFFF01);

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FFIOption<T> {
    None,
    Some(T),
}

impl<T> Into<Option<T>> for FFIOption<T> {
    fn into(self) -> Option<T> {
        match self {
            FFIOption::Some(some) => Some(some),
            FFIOption::None => None,
        }
    }
}

#[repr(C)]
pub struct BootInfo {
    memory_map_ptr: *const MemoryDescriptor,
    memory_map_len: usize,
    runtime_table: SystemTable<Runtime>,
    magic: u32,
    framebuffer: FFIOption<FramebufferPointer>,
}

impl BootInfo {
    pub fn new(
        memory_map: &[MemoryDescriptor],
        runtime_table: SystemTable<Runtime>,
        framebuffer: Option<FramebufferPointer>,
    ) -> Self {
        Self {
            memory_map_ptr: memory_map.as_ptr(),
            memory_map_len: memory_map.len(),
            runtime_table,
            magic: 0xAABB11FF,
            framebuffer: match framebuffer {
                Some(some) => FFIOption::Some(some),
                None => FFIOption::None,
            },
        }
    }

    pub fn memory_map(&mut self) -> &[MemoryDescriptor] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.memory_map_ptr, self.memory_map_len) }
    }

    pub fn runtime_table(&self) -> &SystemTable<Runtime> {
        &self.runtime_table
    }

    pub fn framebuffer_pointer(&self) -> Option<FramebufferPointer> {
        self.framebuffer.into()
    }

    pub fn validate_magic(&self) {
        if self.magic != 0xAABB11FF {
            panic!("boot_info is unaligned, or magic is otherwise corrupted");
        }
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
