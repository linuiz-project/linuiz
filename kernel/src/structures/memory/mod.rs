mod frame;
mod global_allocator;
mod page;

pub mod paging;
pub use frame::*;
pub use global_allocator::*;
pub use page::*;

pub const PAGE_SIZE: usize = 0x1000; // 4096
pub const KIBIBYTE: usize = 0x400; // 1024
pub const MIBIBYTE: usize = KIBIBYTE * KIBIBYTE;

pub fn to_kibibytes(value: usize) -> usize {
    value / KIBIBYTE
}

pub fn to_mibibytes(value: usize) -> usize {
    value / MIBIBYTE
}
