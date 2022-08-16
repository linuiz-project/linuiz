mod slob;

pub use slob::*;

use libkernel::memory::{FrameManager, PageManager};
use libkernel::{Address, Virtual};
use spin::Once;

static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(crate::LIMINE_REV);
static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::LIMINE_REV);

static KERNEL_FRAME_MANAGER: Once<FrameManager> = Once::new();
/// Gets the kernel frame manager.
pub fn get_kernel_frame_manager() -> &'static FrameManager<'static> {
    KERNEL_FRAME_MANAGER.call_once(|| {
        FrameManager::from_mmap(
            LIMINE_MMAP
                .get_response()
                .get()
                .expect("bootloader provided no memory map response")
                .mmap()
                .expect("bootloader provided no memory map entries"),
        )
    })
}

static HHDM_ADDR: Once<Address<Virtual>> = Once::new();
// Gets the kernel's higher half direct mapping page.
pub fn get_kernel_hhdm_addr() -> Address<Virtual> {
    *HHDM_ADDR.call_once(|| {
        Address::<Virtual>::new(
            LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset
                as usize,
        )
        .expect("bootloader provided an invalid higher-half direct mapping address")
    })
}

static KERNEL_PAGE_MANAGER: Once<PageManager> = Once::new();
/// Gets the kernel page manager.
pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER
        .call_once(|| unsafe { PageManager::from_current(&libkernel::memory::Page::from_addr(get_kernel_hhdm_addr())) })
}
