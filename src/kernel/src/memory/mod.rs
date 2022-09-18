pub mod io;
pub mod slab;
pub mod slob;

use libcommon::{Address, Frame, Page, Virtual};
use spin::Once;

struct Mut;
impl InteriorRef for Mut {
    type RefType<'a, T> = &'a mut T where T: 'a;

    fn shared_ref<'a, T>(r: &'a Self::RefType<'_, T>) -> &'a T {
        &**r
    }
}
static KERNEL_ALLOCATOR: Once<slab::SlabAllocator<'static>> = Once::new();

pub unsafe fn init_global_allocator() {
    libcommon::memory::set_global_allocator(
        KERNEL_ALLOCATOR
            .call_once(|| slab::SlabAllocator::from_memory_map(get_limine_mmap(), get_kernel_hhdm_address()).unwrap()),
    )
}

fn get_limine_mmap() -> &'static [limine::NonNullPtr<limine::LimineMemmapEntry>] {
    static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(crate::LIMINE_REV);

    LIMINE_MMAP.get_response().get().expect("bootloader provided no memory map response").memmap()
}

static HHDM_ADDRESS: Once<Address<Virtual>> = Once::new();
/// SAFETY: This function assumes it will be called before the kernel takes ownership of the page tables.
pub unsafe fn init_kernel_hhdm_address() {
    HHDM_ADDRESS.call_once(|| {
        static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::LIMINE_REV);

        Address::<Virtual>::new(
            LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset,
        )
        .expect("bootloader provided a non-canonical higher-half direct mapping address")
    });
}
pub fn get_kernel_hhdm_address() -> Address<Virtual> {
    *HHDM_ADDRESS.get().unwrap()
}

static KERNEL_PAGE_MANAGER: Once<PageManager> = Once::new();
pub fn init_kernel_page_manager() {
    KERNEL_PAGE_MANAGER.call_once(|| {
        // SAFETY: The mapped page is guaranteed to be valid, as the kernel guarantees its HHDM will be valid.
        unsafe {
            PageManager::new(4, get_kernel_hhdm_address(), None, get_kernel_frame_manager())
                .expect("failed to create kernel page manager")
        }
    });
}
pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER.get().unwrap()
}

pub fn reclaim_bootloader_frames() {
    let frame_manager = get_kernel_frame_manager();
    frame_manager.iter().enumerate().filter(|(_, (_, ty))| *ty == FrameType::BootReclaim).for_each(
        |(frame_index, _)| {
            // SAFETY: These frames come directly from the frame manager, and so are guaranteed valid.
            let frame = unsafe { Address::<Frame>::new_unchecked((frame_index * 0x1000) as u64) };
            frame_manager.try_modify_type(frame, FrameType::Usable).ok();
            frame_manager.free(frame).ok();
        },
    );
}

pub fn allocate_pages(page_count: usize) -> *mut u8 {
    // TODO maybe don't `.unwrap()` here, just return the option.
    let base_frame = get_kernel_frame_manager().lock_next_many(page_count).unwrap();

    // SAFETY:  Kernel HHDM is guaranteed (by the kernel) to be valid, so this cannot fail.
    unsafe { get_kernel_hhdm_address().as_mut_ptr::<u8>().add(base_frame.as_usize() * 0x1000) }
}

pub fn ensure_hhdm_frame_is_mapped(frame: Address<Frame>, page_attributes: crate::memory::PageAttributes) {
    let page_manager = crate::memory::get_kernel_page_manager();
    let hhdm_address = get_kernel_hhdm_address();
    let hhdm_page =
        Address::<Page>::new(hhdm_address.as_u64() + frame.as_u64(), libcommon::PageAlign::Align4KiB).unwrap();

    if !page_manager.is_mapped(hhdm_page) {
        let frame_manager = crate::memory::get_kernel_frame_manager();
        frame_manager.lock(frame).ok();
        page_manager.map(hhdm_page, frame, false, page_attributes, frame_manager).unwrap();
    }

    assert!(page_manager.is_mapped(hhdm_page));
}
