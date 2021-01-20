mod bump_allocator;
mod global_memory;

pub use bump_allocator::*;
pub use global_memory::*;

use x86_64::VirtAddr;

pub unsafe trait Allocator {
    unsafe fn alloc<R: Sized>(&self) -> R;
    unsafe fn malloc(&self, size: usize) -> VirtAddr;
    unsafe fn dealloc(&self, addr: VirtAddr, size: usize);
}
