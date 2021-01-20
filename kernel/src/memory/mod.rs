mod frame;
mod page;

pub mod allocators;
pub mod paging;
pub use frame::*;
pub use page::*;

use efi_boot::MemoryType;

pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub fn is_reservable_memory_type(ty: MemoryType) -> bool {
    match ty {
        MemoryType::LOADER_CODE
        | MemoryType::LOADER_DATA
        | MemoryType::BOOT_SERVICES_CODE
        | MemoryType::BOOT_SERVICES_DATA
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
