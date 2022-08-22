mod slob;

use core::cell::SyncUnsafeCell;

pub use slob::*;

use libkernel::memory::{FrameManager, PageManager};
use libkernel::{Address, Virtual};
use spin::Once;

fn get_limine_mmap() -> &'static [limine::LimineMemmapEntry] {
    static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(crate::LIMINE_REV);

    LIMINE_MMAP
        .get_response()
        .get()
        .expect("bootloader provided no memory map response")
        .mmap()
        .expect("bootloader provided no memory map entries")
}

pub static HHDM_ADDRESS: Once<Address<Virtual>> = Once::new();
/// SAFETY: This function assumes it will be called before the kernel takes ownership of the page tables.
pub unsafe fn init_kernel_hhdm_address() {
    HHDM_ADDRESS.call_once(|| {
        static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::LIMINE_REV);

        Address::<Virtual>::new(
            LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset
                as usize,
        )
        .expect("bootloader provided an invalid higher-half direct mapping address")
    });
}
pub fn get_kernel_hhdm_address() -> Address<Virtual> {
    unsafe { *HHDM_ADDRESS.get().unwrap() }
}

static KERNEL_FRAME_MANAGER: Once<FrameManager> = Once::new();
/// SAFETY: This function assumes it will be called before the kernel takes ownership of the page tables.
pub unsafe fn init_kernel_frame_manager() {
    KERNEL_FRAME_MANAGER.call_once(|| FrameManager::from_mmap(get_limine_mmap(), get_kernel_hhdm_address()));
}
pub fn get_kernel_frame_manager() -> &'static FrameManager<'static> {
    KERNEL_FRAME_MANAGER.get().unwrap()
}

static KERNEL_PAGE_MANAGER: Once<PageManager> = Once::new();
pub fn init_kernel_page_manager() {
    KERNEL_PAGE_MANAGER.call_once(|| unsafe {
        PageManager::new(
            get_kernel_frame_manager(),
            &libkernel::memory::Page::from_index(get_kernel_hhdm_address().page_index()),
            None,
        )
    });
}
pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER.get().unwrap()
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
    let base_frame_index = get_kernel_frame_manager().lock_next_many(page_count).unwrap();

    unsafe { get_kernel_hhdm_address().as_mut_ptr::<u8>().add(base_frame_index * 0x1000) }
}
