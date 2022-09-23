mod paging;
mod virtual_mapper;

pub mod io;
pub mod slab;
pub use libarch::memory::PageAttributes;
pub use paging::*;
pub use virtual_mapper::*;

use libcommon::{Address, Virtual};
use spin::Once;

static KERNEL_ALLOCATOR: Once<slab::SlabAllocator<'static>> = Once::new();
pub unsafe fn init_global_allocator(memory_map: &[crate::MmapEntry]) {
    libcommon::memory::set_global_allocator(
        KERNEL_ALLOCATOR
            .call_once(|| slab::SlabAllocator::from_memory_map(memory_map, get_kernel_hhdm_address()).unwrap()),
    )
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

static KERNEL_PAGE_MANAGER: Once<VirtualMapper> = Once::new();
pub fn init_kernel_page_manager() {
    KERNEL_PAGE_MANAGER.call_once(|| {
        // SAFETY: The mapped page is guaranteed to be valid, as the kernel guarantees its HHDM will be valid.
        unsafe { VirtualMapper::new(4, get_kernel_hhdm_address(), None).expect("failed to create kernel page manager") }
    });
}
pub fn get_kernel_virtual_mapper() -> &'static VirtualMapper {
    KERNEL_PAGE_MANAGER.get().unwrap()
}

// TODO this
// pub fn reclaim_bootloader_frames() {
//     let frame_manager = get_kernel_frame_manager();
//     frame_manager.iter().enumerate().filter(|(_, (_, ty))| *ty == FrameType::BootReclaim).for_each(
//         |(frame_index, _)| {
//             // SAFETY: These frames come directly from the frame manager, and so are guaranteed valid.
//             let frame = unsafe { Address::<Frame>::new_unchecked((frame_index * 0x1000) as u64) };
//             frame_manager.try_modify_type(frame, FrameType::Usable).ok();
//             frame_manager.free(frame).ok();
//         },
//     );
// }
