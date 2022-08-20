mod slob;

pub use slob::*;

use libkernel::memory::{FrameManager, PageManager};
use libkernel::{Address, Virtual};
use spin::Once;

static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(crate::LIMINE_REV);
static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::LIMINE_REV);

fn get_limine_mmap() -> &'static [limine::LimineMemmapEntry] {
    LIMINE_MMAP
        .get_response()
        .get()
        .expect("bootloader provided no memory map response")
        .mmap()
        .expect("bootloader provided no memory map entries")
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

static KERNEL_FRAME_MANAGER: Once<FrameManager> = Once::new();
/// Gets the kernel frame manager.
pub fn get_kernel_frame_manager() -> &'static FrameManager<'static> {
    KERNEL_FRAME_MANAGER.call_once(|| FrameManager::from_mmap(get_limine_mmap(), get_kernel_hhdm_addr()))
}

static KERNEL_PAGE_MANAGER: Once<PageManager> = Once::new();

pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER.call_once(|| unsafe {
        PageManager::new(
            get_kernel_frame_manager(),
            &libkernel::memory::Page::from_index(get_kernel_hhdm_addr().page_index()),
            None,
        )
    })
}

pub fn reclaim_bootloader_memory() {
    use libkernel::memory::FrameType;

    let frame_manager = get_kernel_frame_manager();
    frame_manager.iter().enumerate().filter(|(_, (_, ty))| *ty == FrameType::BootReclaim).for_each(
        |(frame_index, (_, ty))| {
            frame_manager.force_modify_type(frame_index, FrameType::Usable).ok();
            frame_manager.free(frame_index).ok();
        },
    );
}

pub fn allocate_pages(page_count: usize) -> *mut u8 {
    0x0 as *mut u8
}
