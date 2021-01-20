#![no_std]
#![feature(abi_efiapi)]
#![feature(core_intrinsics)]

pub mod elf;
pub mod memory;

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
    memory_map_ptr: *const u8,
    memory_map_len: usize,
    magic: u32,
    framebuffer: FFIOption<FramebufferPointer>,
}

impl BootInfo {
    pub fn new(memory_map: &[u8], framebuffer: Option<FramebufferPointer>) -> Self {
        Self {
            memory_map_ptr: memory_map.as_ptr(),
            memory_map_len: memory_map.len(),
            magic: 0xAABB11FF,
            framebuffer: match framebuffer {
                Some(some) => FFIOption::Some(some),
                None => FFIOption::None,
            },
        }
    }

    pub fn memory_map<R: Sized>(&self) -> &[R] {
        unsafe {
            &*core::ptr::slice_from_raw_parts(
                self.memory_map_ptr as *const R,
                self.memory_map_len / core::mem::size_of::<R>(),
            )
        }
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

pub fn align_up(value: usize, alignment: usize) -> usize {
    assert!(
        alignment.is_power_of_two(),
        "`alignment` must be a power of two"
    );

    let alignment_mask = alignment - 1;
    if value & alignment_mask == 0 {
        value
    } else {
        (value | alignment_mask) + 1
    }
}

pub fn align_down(value: usize, alignment: usize) -> usize {
    assert!(
        alignment.is_power_of_two(),
        "alignment must be a power of two"
    );

    value & !(alignment - 1)
}

pub type KernelMain = extern "win64" fn(crate::BootInfo) -> usize;

#[macro_export]
macro_rules! entrypoint {
    ($path:path) => {
        #[export_name = "_start"]
        pub extern "win64" fn __impl_kernel_main(boot_info: $crate::BootInfo) -> usize {
            let function: $crate::KernelMain = $path;
            function(boot_info)
        }
    };
}
