mod paging;

pub mod io;
pub use paging::*;
pub mod address_space;
pub mod pmm;

use crate::{exceptions::Exception, interrupts::InterruptCell, local_state::do_catch};
use address_space::Mapper;
use alloc::{alloc::Global, string::String};
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
};
use lzstd::{Address, Frame, PAGE_MASK, PAGE_SHIFT, PAGE_SIZE, TABLE_INDEX_SHIFT};
use slab::SlabAllocator;
use spin::{Lazy, Mutex, Once};
use try_alloc::boxed::TryBox;

const VIRT_CANONICAL_SHIFT: u32 =
    ((TABLE_INDEX_SHIFT.get() * PageDepth::MAX.get().get()) + PAGE_SHIFT.get()).checked_sub(1).unwrap();
const VIRT_CANONICAL_BITS: usize = usize::MAX >> VIRT_CANONICAL_SHIFT;

const fn checked_virt_canonical(address: usize) -> bool {
    matches!(address >> VIRT_CANONICAL_SHIFT, 0 | VIRT_CANONICAL_BITS)
}

const fn virt_truncate(address: usize) -> usize {
    (((address << 16) as isize) >> 16) as usize
}

pub struct Virtual;
impl lzstd::AddressKind for Virtual {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        checked_virt_canonical(init).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        virt_truncate(init)
    }
}
impl lzstd::PtrableAddressKind for Virtual {
    fn from_ptr<T>(ptr: *mut T) -> Self::ReprType {
        ptr.addr()
    }

    fn as_ptr(repr: Self::ReprType) -> *mut u8 {
        repr as *mut u8
    }
}

pub struct Page;
impl lzstd::AddressKind for Page {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        (((init & PAGE_MASK) == 0) && checked_virt_canonical(init)).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        init & !&!PAGE_MASK
    }
}
impl lzstd::PtrableAddressKind for Page {
    fn from_ptr<T>(ptr: *mut T) -> Self::ReprType {
        ptr.addr()
    }

    fn as_ptr(repr: Self::ReprType) -> *mut u8 {
        repr as *mut u8
    }
}
impl lzstd::IndexableAddressKind for Page {
    fn from_index(index: usize) -> Option<Self::ReprType> {
        (index <= !(VIRT_CANONICAL_BITS >> PAGE_SHIFT.get())).then_some(index << PAGE_SHIFT.get())
    }

    fn index(repr: Self::ReprType) -> usize {
        repr >> PAGE_SHIFT.get()
    }
}

pub fn hhdm_address() -> Address<Virtual> {
    static HHDM_ADDRESS: Once<Address<Virtual>> = Once::new();

    *HHDM_ADDRESS.call_once(|| {
        static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::boot::LIMINE_REV);

        Address::new(
            LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset
                as usize,
        )
        .expect("bootloader provided a non-canonical higher-half direct mapping address")
    })
}

pub fn with_kmapper<T>(func: impl FnOnce(&mut Mapper) -> T) -> T {
    static KERNEL_MAPPER: Once<InterruptCell<Mutex<Mapper>>> = Once::new();

    KERNEL_MAPPER
        .call_once(|| InterruptCell::new(Mutex::new(Mapper::new().expect("failed to create kernel space mapper"))))
        .with(|mapper| {
            let mut mapper = mapper.lock();
            func(&mut *mapper)
        })
}

#[cfg(target_arch = "x86_64")]
pub struct PagingRegister(pub Address<Frame>, pub crate::arch::x64::registers::control::CR3Flags);
#[cfg(target_arch = "riscv64")]
pub struct VmemRegister(pub Address<Frame>, pub u16, pub crate::arch::rv64::registers::satp::Mode);

impl PagingRegister {
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

pub type PhysicalAllocator = &'static pmm::PhysicalMemoryManager<'static>;

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
        hhdm_address(),
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

pub unsafe fn catch_read(ptr: NonNull<[u8]>) -> Result<TryBox<[u8]>, Exception> {
    let mem_range = ptr.as_uninit_slice().as_ptr_range();
    let aligned_start = lzstd::align_down(mem_range.start.addr(), NonZeroUsize::new(PAGE_SIZE).unwrap());
    let mem_end = mem_range.end.addr();

    let mut copied_mem = TryBox::new_slice(ptr.len(), 0u8).unwrap();
    for (offset, page_addr) in (aligned_start..mem_end).enumerate().step_by(PAGE_SIZE) {
        let ptr_addr = core::cmp::max(mem_range.start.addr(), page_addr);
        let ptr_len = core::cmp::min(mem_end.saturating_sub(ptr_addr), PAGE_SIZE);

        // Safety: Box slice and this iterator are bound by the ptr len.
        let to_ptr = unsafe { (&mut copied_mem).as_mut_ptr().add(offset) };
        // Safety: Copy is only invalid if the caller provided an invalid pointer.
        do_catch(|| unsafe {
            core::ptr::copy_nonoverlapping(ptr_addr as *mut u8, to_ptr, ptr_len);
        })?;
    }

    Ok(copied_mem)
}

// TODO TryString
pub unsafe fn catch_read_str(mut read_ptr: NonNull<u8>) -> Result<String, Exception> {
    let mut string = String::new();

    'y: loop {
        let read_len = read_ptr.as_ptr().align_offset(PAGE_SIZE);
        read_ptr = NonNull::new(unsafe { read_ptr.as_ptr().add(read_len) }).unwrap();

        for ch in catch_read(NonNull::slice_from_raw_parts(read_ptr, read_len))?.into_iter().copied().map(char::from) {
            if ch == '\0' {
                break 'y;
            } else {
                string.push(ch);
            }
        }
    }

    Ok(string)
}
