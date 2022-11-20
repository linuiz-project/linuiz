mod mapper;
mod paging;

pub mod io;
pub use mapper::*;
pub use paging::*;
pub mod pmm;
pub mod slob;

use alloc::alloc::Global;
use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
};
use lzstd::{Address, Frame, Virtual};
use slab::SlabAllocator;
use spin::{Lazy, Once};
use try_alloc::boxed::TryBox;

pub fn get_hhdm_address() -> Address<Virtual> {
    static HHDM_ADDRESS: Once<Address<Virtual>> = Once::new();

    HHDM_ADDRESS
        .call_once(|| {
            static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::boot::LIMINE_REV);

            Address::<Virtual>::new(
                LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset,
            )
            .expect("bootloader provided a non-canonical higher-half direct mapping address")
        })
        .clone()
}

pub fn get_kernel_mapper() -> &'static Mapper {
    static KERNEL_MAPPER: Once<Mapper> = Once::new();

    KERNEL_MAPPER.call_once(|| {
        // ### Safety: The kernel guarantees the HHDM will be valid.
        unsafe { Mapper::new(4, get_hhdm_address(), None).unwrap() }
    })
}

#[cfg(target_arch = "x86_64")]
pub struct VmemRegister(pub Address<Frame>, pub crate::arch::x64::registers::control::CR3Flags);
#[cfg(target_arch = "riscv64")]
pub struct VmemRegister(pub Address<Frame>, pub u16, pub crate::arch::rv64::registers::satp::Mode);

impl VmemRegister {
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

    /// ### Safety
    ///
    /// Writing to this register has the chance to externally invalidate memory references.
    pub unsafe fn write(args: &Self) {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(args.0, args.1);

        #[cfg(target_arch = "riscv64")]
        crate::arch::rv64::registers::satp::write(args.0.as_usize(), args.1, args.2);
    }

    #[inline]
    pub const fn frame(&self) -> Address<Frame> {
        self.0
    }
}

pub fn supports_5_level_paging() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::cpuid::EXT_FEATURE_INFO
            .as_ref()
            .map(|ext_feature_info| ext_feature_info.has_la57())
            .unwrap_or(false)
    }

    #[cfg(target_arch = "riscv64")]
    {
        todo!()
    }
}

pub fn is_5_level_paged() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        supports_5_level_paging()
            && crate::arch::x64::registers::control::CR4::read()
                .contains(crate::arch::x64::registers::control::CR4Flags::LA57)
    }
}

pub static PMM: Lazy<pmm::PhysicalMemoryManager> = Lazy::new(|| unsafe {
    let memory_map = crate::boot::get_memory_map().unwrap();
    pmm::PhysicalMemoryManager::from_memory_map(
        memory_map.iter().map(|entry| pmm::MemoryMapping {
            base: entry.base as usize,
            len: entry.len as usize,
            typ: {
                use limine::LimineMemoryMapEntryType;
                use pmm::FrameType;

                match entry.typ {
                    LimineMemoryMapEntryType::Usable => FrameType::Generic,
                    LimineMemoryMapEntryType::BootloaderReclaimable => FrameType::BootReclaim,
                    LimineMemoryMapEntryType::AcpiReclaimable => FrameType::AcpiReclaim,
                    LimineMemoryMapEntryType::KernelAndModules
                    | LimineMemoryMapEntryType::Reserved
                    | LimineMemoryMapEntryType::AcpiNvs
                    | LimineMemoryMapEntryType::Framebuffer => FrameType::Reserved,
                    LimineMemoryMapEntryType::BadMemory => FrameType::Unusable,
                }
            },
        }),
        core::ptr::NonNull::new(get_hhdm_address().as_mut_ptr()).unwrap(),
    )
    .unwrap()
});

pub static KMALLOC: Lazy<SlabAllocator<&pmm::PhysicalMemoryManager>> = Lazy::new(|| SlabAllocator::new_in(11, &*PMM));

mod global_allocator_impl {
    use super::KMALLOC;
    use core::{
        alloc::{Allocator, GlobalAlloc, Layout},
        ptr::NonNull,
    };

    struct GlobalAllocator;

    unsafe impl GlobalAlloc for GlobalAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            KMALLOC.allocate(layout).map_or(core::ptr::null_mut(), |ptr| ptr.as_non_null_ptr().as_ptr())
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            KMALLOC.deallocate(NonNull::new(ptr).unwrap(), layout);
        }
    }

    unsafe impl Allocator for GlobalAllocator {
        fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, core::alloc::AllocError> {
            KMALLOC.allocate(layout)
        }

        unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            KMALLOC.deallocate(ptr, layout);
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator;
}

pub unsafe fn out_of_memory() -> ! {
    panic!("Kernel ran out of memory during initialization.")
}

pub type Stack = TryBox<[core::mem::MaybeUninit<u8>], AlignedAllocator<0x10>>;

pub fn allocate_kernel_stack<const SIZE: usize>() -> Result<Stack, AllocError> {
    TryBox::new_uninit_slice_in(SIZE, AlignedAllocator::new())
}

pub struct AlignedAllocator<const ALIGN: usize, A: Allocator = Global>(A);

impl<const ALIGN: usize> AlignedAllocator<ALIGN> {
    #[inline]
    pub const fn new() -> Self {
        AlignedAllocator::new_in(Global)
    }
}

impl<const ALIGN: usize, A: Allocator> AlignedAllocator<ALIGN, A> {
    #[inline]
    pub const fn new_in(allocator: A) -> Self {
        Self(allocator)
    }
}

/// # Safety: Type is merely a wrapper for aligned allocation of another allocator impl.
unsafe impl<const ALIGN: usize, A: Allocator> Allocator for AlignedAllocator<ALIGN, A> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate(layout),
            Err(_) => Err(AllocError),
        }
    }

    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        match layout.align_to(ALIGN) {
            Ok(layout) => self.0.allocate_zeroed(layout),
            Err(_) => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        match layout.align_to(ALIGN) {
            // ### Safety: This function shares the same invariants as `GlobalAllocator::deallocate`.
            Ok(layout) => unsafe { self.0.deallocate(ptr, layout) },
            Err(_) => unimplemented!(),
        }
    }
}
