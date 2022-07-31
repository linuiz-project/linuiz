mod slob;

pub use slob::*;

use libkernel::{
    cell::SyncOnceCell,
    memory::{FrameManager, PageManager},
    Address, Virtual,
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

#[derive(Debug)]
pub struct KernelFrameManagerNotSet;
/// Gets the kernel frame manager.
pub fn get_kernel_frame_manager() -> Result<&'static FrameManager<'static>, KernelFrameManagerNotSet> {
    match KERNEL_FRAME_MANAGER.get() {
        Some(frame_manager) => Ok(frame_manager),
        None => Err(KernelFrameManagerNotSet),
    }
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

#[derive(Debug)]
pub struct HHDMAddrNotSet;
// Gets the kernel's higher half direct mapping page.
pub fn get_kernel_hhdm_addr() -> Result<Address<Virtual>, HHDMAddrNotSet> {
    HHDM_ADDR.get().map(|addr| *addr).ok_or(HHDMAddrNotSet)
}

#[derive(Debug)]
pub struct KernelPageManagerNotSet;
/// Gets the kernel page manager.
pub fn get_kernel_page_manager() -> Result<&'static PageManager, KernelPageManagerNotSet> {
    match KERNEL_PAGE_MANAGER.get() {
        Some(page_manager) => Ok(page_manager),
        None => Err(KernelPageManagerNotSet),
    }
}
