mod frame_manager;
mod page_manager;
mod paging;

pub mod io;
pub mod slob;

pub use frame_manager::*;
pub use page_manager::*;
pub use paging::*;

use core::{alloc::GlobalAlloc, cell::OnceCell};
use libkernel::{Address, Virtual};
use spin::Once;

struct GlobalAllocator<'m>(OnceCell<&'m dyn GlobalAlloc>);
// SAFETY: `GlobalAlloc` trait requires `Send`.
unsafe impl Send for GlobalAllocator<'_> {}
// SAFETY: `GlobalAlloc` trait requires `Sync`.
unsafe impl Sync for GlobalAllocator<'_> {}

/// SAFETY: This struct is a simple wrapper around `GlobalAlloc` itself, and so necessarily implements its safety invariants.
unsafe impl GlobalAlloc for GlobalAllocator<'_> {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        match self.0.get() {
            Some(global_allocator) => global_allocator.alloc(layout),
            // TODO properly handle abort, via `ud2` handler and perhaps an interrupt flag in fsbase MSR?
            None => core::intrinsics::abort(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        match self.0.get() {
            Some(global_allocator) => global_allocator.dealloc(ptr, layout),
            None => core::intrinsics::abort(),
        }
    }
}

#[global_allocator]
static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator(OnceCell::new());

pub unsafe fn set_global_allocator(galloc: &'static dyn GlobalAlloc) {
    if GLOBAL_ALLOCATOR.0.set(galloc).is_err() {
        error!("Global allocator is already set.");
        crate::interrupts::wait_loop();
    }
}

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
    *HHDM_ADDRESS.get().unwrap()
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
    KERNEL_PAGE_MANAGER.call_once(|| {
        let frame_manager = get_kernel_frame_manager();
        let mapped_page = libkernel::memory::Page::from_addr(get_kernel_hhdm_address());
        let pml4_copy = None;

        // SAFETY:  The mapped page is guaranteed to be valid, as the kernel guarantees its HHDM will be valid.
        unsafe { PageManager::new(frame_manager, &mapped_page, pml4_copy) }
    });
}
pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER.get().unwrap()
}

pub fn reclaim_bootloader_frames() {
    let frame_manager = get_kernel_frame_manager();
    frame_manager.iter().enumerate().filter(|(_, (_, ty))| *ty == FrameType::BootReclaim).for_each(
        |(frame_index, _)| {
            frame_manager.force_modify_type(frame_index, FrameType::Usable).ok();
            frame_manager.free(frame_index).ok();
        },
    );
}

pub fn allocate_pages(page_count: usize) -> *mut u8 {
    let base_frame_index = get_kernel_frame_manager().lock_next_many(page_count).unwrap();

    // SAFETY:  Kernel HHDM is guaranteed (by the kernel) to be valid, so this cannot fail.
    unsafe { get_kernel_hhdm_address().as_mut_ptr::<u8>().add(base_frame_index * 0x1000) }
}

#[cfg(target_arch = "x86_64")]
pub struct RootPageTable(pub Address<libkernel::Physical>, pub crate::arch::x64::registers::control::CR3Flags);
#[cfg(target_arch = "riscv64")]
pub struct RootPageTable(pub Address<libkernel::Physical>, pub u16, pub crate::arch::rv64::registers::satp::Mode);

impl RootPageTable {
    pub fn read() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let args = crate::arch::x64::registers::control::CR3::read();
            Self(args.0, args.1)
        }

        #[cfg(target_arch = "riscv64")]
        {
            let args = crate::arch::rv64::registers::satp::read();
            Self(args.0, args.1, args.2)
        }
    }

    /// SAFETY: Writing to this register has the chance to externally invalidate memory references.
    pub unsafe fn write(args: &Self) {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(args.0, args.1);
    }
}
