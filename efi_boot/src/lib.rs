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
pub struct BootInfo<MM> {
    memory_map_ptr: *const MM,
    memory_map_len: usize,
    magic: u32,
    framebuffer: FFIOption<FramebufferPointer>,
}

impl<MM> BootInfo<MM> {
    pub fn new(memory_map: &[MM], framebuffer: Option<FramebufferPointer>) -> Self {
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

    pub fn memory_map(&self) -> &[MM] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.memory_map_ptr, self.memory_map_len) }
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

pub type KernelMain<MM> = extern "win64" fn(crate::BootInfo<MM>) -> !;
