mod frame;
mod page;
mod uefi;

pub mod allocators;
pub mod paging;
pub use frame::*;
pub use page::*;
pub use uefi::*;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
    RESERVEDVED,
    LOADER_CODE,
    LOADER_DATA,
    BOOT_SERVICES_CODE,
    BOOT_SERVICES_DATA,
    RUNTIME_SERVICES_CODE,
    RUNTIME_SERVICES_DATA,
    CONVENTIONAL,
    UNUSABLE,
    ACPI_RECLAIM,
    ACPI_NON_VOLATILE,
    MMIO,
    MMIO_PORT_SPACE,
    PAL_CODE,
    PERSISTENT_MEMORY,
    KERNEL_CODE = 0xFFFFFF00,
    KERNEL_DATA = 0xFFFFFF01,
}

pub fn is_reserved_memory_type(mem_type: MemoryType) -> bool {
    match mem_type {
        MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA
        | MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA
        | MemoryType::CONVENTIONAL => false,
        _ => true,
    }
}

pub fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}
