mod slob;

pub use slob::*;

use libarch::{Address, Virtual};
use libkernel::{
    cell::SyncOnceCell,
    memory::{FrameManager, PageManager},
};

static KERNEL_FRAME_MANAGER: SyncOnceCell<FrameManager> = unsafe { SyncOnceCell::new() };

/// Sets the kernel frame manager.
pub fn init_kernel_frame_manager(memory_map: &[limine::LimineMemmapEntry]) {
    // Explicitly ensure kernel frame manager has not been set, to avoid creating an entirely new
    // FrameManager structure (which would be necessary for check if we used the `Result` from `.set()`).
    if let None = KERNEL_FRAME_MANAGER.get() {
        KERNEL_FRAME_MANAGER.set(FrameManager::from_mmap(memory_map)).ok();
    } else {
        panic!("Kernel frame manager already exists!");
    }
}

/// Gets the kernel frame manager.
pub fn get_kernel_frame_manager() -> Option<&'static FrameManager<'static>> {
    KERNEL_FRAME_MANAGER.get()
}

static HHDM_ADDR: SyncOnceCell<Address<Virtual>> = unsafe { SyncOnceCell::new() };
static KERNEL_PAGE_MANAGER: SyncOnceCell<PageManager> = unsafe { SyncOnceCell::new() };

/// Sets the kernel page manager.
pub fn init_kernel_page_manager(hhdm_addr: Address<Virtual>) {
    if let Err(_) = HHDM_ADDR.set(hhdm_addr) {
        panic!("Kernel higher-half direct mapping address already set!");
    }

    // Explicitly ensure kernel page manager has not been set, to avoid creating an entirely new
    // PageManager structure (which would be necessary for check if we used the `Result` from `.set()`).
    if let None = KERNEL_PAGE_MANAGER.get() {
        KERNEL_PAGE_MANAGER
            .set(unsafe { PageManager::from_current(&libkernel::memory::Page::from_addr(hhdm_addr)) })
            .ok();
    } else {
        panic!("Kernel page manager already exists.");
    }
}

// Gets the kernel's higher half direct mapping page.
pub fn get_kernel_hhdm_addr() -> Option<Address<Virtual>> {
    HHDM_ADDR.get().map(|addr| *addr)
}

/// Gets the kernel page manager.
pub fn get_kernel_page_manager() -> Option<&'static PageManager> {
    KERNEL_PAGE_MANAGER.get()
}
