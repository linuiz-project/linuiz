use core::{
    intrinsics::{wrapping_add, wrapping_mul, wrapping_sub},
    ptr::slice_from_raw_parts_mut,
};
use uefi::{
    prelude::BootServices,
    table::boot::{AllocateType, MemoryType},
};

pub const PAGE_SIZE: usize = 0x1000; // 4096
                                     // TODO some sort of checking to ensure we have enough address
                                     // space to actually load the kernel

pub struct PointerBuffer<'buf> {
    pub pointer: *mut u8,
    pub buffer: &'buf mut [u8],
}

pub fn allocate_pool(
    boot_services: &BootServices,
    memory_type: MemoryType,
    size: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pool(memory_type, size) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer = unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, size) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

pub fn allocate_pages(
    boot_services: &BootServices,
    allocate_type: AllocateType,
    memory_type: MemoryType,
    pages_count: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pages(allocate_type, memory_type, pages_count)
    {
        Ok(completion) => completion.unwrap() as *mut u8,
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer =
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, pages_count * PAGE_SIZE) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

pub fn free_pool(boot_services: &BootServices, buffer: PointerBuffer) {
    match boot_services.free_pool(buffer.pointer) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

pub fn free_pages(boot_services: &BootServices, buffer: PointerBuffer, count: usize) {
    match boot_services.free_pages(buffer.pointer as u64, count) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

pub fn align_up(value: usize, alignment: usize) -> usize {
    let super_aligned = wrapping_add(value, alignment);
    let force_under_aligned = wrapping_sub(super_aligned, 1);
    wrapping_mul(force_under_aligned / alignment, alignment)
}

pub fn align_down(value: usize, alignment: usize) -> usize {
    (value / alignment) * alignment
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
pub fn aligned_slices(size: usize, alignment: usize) -> usize {
    ((size + alignment) - 1) / alignment
}
