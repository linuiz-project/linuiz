mod frame_manager;
mod page_manager;

pub use frame_manager::*;
pub use page_manager::*;
pub use paging::*;

pub mod paging;
pub mod uefi;
pub mod volatile;

#[cfg(feature = "global_allocator")]
pub mod global_alloc {
    use core::{alloc::GlobalAlloc, lazy::OnceCell};

    struct GlobalAllocator<'m>(OnceCell<&'m dyn GlobalAlloc>);

    impl GlobalAllocator<'_> {
        pub const fn new() -> Self {
            Self(OnceCell::new())
        }
    }

    unsafe impl Send for GlobalAllocator<'_> {}
    unsafe impl Sync for GlobalAllocator<'_> {}

    unsafe impl GlobalAlloc for GlobalAllocator<'_> {
        unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
            self.0.get().expect("no global allocator").alloc(layout)
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
            self.0
                .get()
                .expect("no global allocator")
                .dealloc(ptr, layout);
        }
    }

    #[global_allocator]
    static GLOBAL_ALLOCATOR: GlobalAllocator = GlobalAllocator::new();

    pub unsafe fn set(galloc: &'static dyn GlobalAlloc) {
        GLOBAL_ALLOCATOR
            .0
            .set(galloc)
            .map_err(|_| {
                panic!("global allocator already set");
            })
            .unwrap();
    }
}

use crate::cell::SyncOnceCell;

static FRAME_MANAGER: SyncOnceCell<FrameManager> = SyncOnceCell::new();
static PAGE_MANAGER: SyncOnceCell<PageManager> = SyncOnceCell::new();

/// Initializes global memory structures (frame & page managers).
///
/// This function *does not* swap the current page table. To commit the page manager
/// to CR3 and map physical memory at the correct offset, call `finalize_paging()`.
pub fn init(memory_map: &[uefi::MemoryDescriptor]) {
    FRAME_MANAGER
        .set(FrameManager::from_mmap(memory_map))
        .map_err(|_| {
            panic!("frame manager has already been initialized");
        })
        .unwrap();

    let frame_manager = FRAME_MANAGER.get().unwrap();
    let page_manager = unsafe { PageManager::new(&Page::null(), None) };

    // Set page attributes for UEFI descriptor pages.
    for descriptor in memory_map.iter().skip(memory_map.len()) {
        if descriptor.range().contains(&0x3f07540) {
            info!("{:?}", descriptor);
        }

        continue;

        let mut page_attribs = PageAttributes::empty();

        use crate::memory::uefi::{MemoryAttributes, MemoryType};

        if descriptor.att.contains(MemoryAttributes::WRITE_THROUGH) {
            page_attribs.insert(PageAttributes::WRITABLE);
            page_attribs.insert(PageAttributes::WRITE_THROUGH);
        }

        if descriptor.att.contains(MemoryAttributes::WRITE_BACK) {
            page_attribs.insert(PageAttributes::WRITABLE);
            page_attribs.remove(PageAttributes::WRITE_THROUGH);
        }

        if descriptor.att.contains(MemoryAttributes::EXEC_PROTECT) {
            page_attribs.insert(PageAttributes::NO_EXECUTE);
        }

        if descriptor.att.contains(MemoryAttributes::UNCACHEABLE) {
            page_attribs.insert(PageAttributes::UNCACHEABLE);
        }

        if descriptor.att.contains(MemoryAttributes::READ_ONLY) {
            page_attribs.remove(PageAttributes::WRITABLE);
            page_attribs.remove(PageAttributes::WRITE_THROUGH);
        }

        // If the descriptor type is not unusable...
        if !matches!(
            descriptor.ty,
            MemoryType::UNUSABLE | MemoryType::UNACCEPTED | MemoryType::KERNEL
        ) {
            // ... then iterate its pages and identity map them.
            //     This specific approach allows the memory usage to be decreased overall,
            //     since unused/unusable pages or descriptors will not be mapped.
            for page in descriptor
                .frame_range()
                .map(|index| Page::from_index(index))
            {
                page_manager
                    .identity_map(
                        &page,
                        PageAttributes::PRESENT | PageAttributes::GLOBAL | page_attribs,
                    )
                    .unwrap();
            }
        }
    }

    for page in frame_manager
        .iter()
        .enumerate()
        .filter_map(|(frame_index, (ty, _, _))| {
            if ty == FrameType::FrameMap {
                Some(Page::from_index(frame_index))
            } else {
                None
            }
        })
    {
        page_manager
            .identity_map(
                &page,
                PageAttributes::PRESENT
                    | PageAttributes::GLOBAL
                    | PageAttributes::NO_EXECUTE
                    | PageAttributes::WRITABLE,
            )
            .unwrap();
    }

    PAGE_MANAGER
        .set(page_manager)
        .map_err(|_| {
            panic!("page manager has already been initialized");
        })
        .unwrap();
}

/// Finalizes the kernel paging structure, and writes it to CR3. Call this
/// after all relevant changes have been made to the global page manager.
///
/// This function should always be called *after* `init()`.
pub fn finalize_paging() {
    unsafe {
        let frame_manager = global_fmgr();
        let page_manager = global_pgmr();

        debug!(
            "Physical memory offset: @{:?}",
            frame_manager.phys_mem_offset()
        );
        page_manager.modify_mapped_page(Page::from_addr(frame_manager.phys_mem_offset()));
        debug!("Writing baseline kernel PML4 to CR3.");
        page_manager.write_cr3();
        debug!("Successfully wrote to CR3.");
    }
}

pub fn global_fmgr() -> &'static FrameManager<'static> {
    FRAME_MANAGER
        .get()
        .expect("kernel frame manager has not been initialized")
}

pub fn global_pgmr() -> &'static PageManager {
    PAGE_MANAGER
        .get()
        .expect("kernel page manager has not been initialized")
}

pub fn alloc_stack(page_count: usize, is_userspace: bool) -> *mut () {
    unsafe {
        let stack_len = page_count * 0x1000;
        let stack_bottom = alloc::alloc::alloc_zeroed(
            core::alloc::Layout::from_size_align(stack_len, 0x1000).unwrap(),
        );
        let stack_top = stack_bottom.add(stack_len);

        let page_manager = global_pgmr();
        for page in Page::range(
            (stack_bottom as usize) / 0x1000,
            (stack_top as usize) / 0x1000,
        ) {
            page_manager.set_page_attribs(
                &page,
                PageAttributes::PRESENT
                    | PageAttributes::WRITABLE
                    | PageAttributes::NO_EXECUTE
                    | if is_userspace {
                        PageAttributes::USERSPACE
                    } else {
                        PageAttributes::empty()
                    },
                AttributeModify::Set,
            );
        }

        stack_top as *mut ()
    }
}

pub struct MMIO {
    ptr: *mut u8,
    len: usize,
}

impl Drop for MMIO {
    fn drop(&mut self) {
        // Possibly reset frame_range? We don't want to forever lose the pointed-to frames, especially if
        // the frames were locked MMIO in error.

        unsafe {
            alloc::alloc::dealloc(
                self.ptr,
                core::alloc::Layout::from_size_align(self.len, 0x1000).unwrap(),
            )
        };
    }
}

impl MMIO {
    /// Creates a new MMIO structure wrapping the given region.
    ///
    /// SAFETY: The caller must ensure that the indicated memory region passed as parameters
    ///         `frame_index` and `count` is valid for MMIO.
    pub unsafe fn new(frame_index: usize, count: usize) -> Self {
        let frame_manager = global_fmgr();

        for frame_index in frame_index..(frame_index + count) {
            info!("{:?}", frame_manager.map_pages().nth(frame_index));

            if let Err(FrameError::TypeConversion { from, to }) =
                frame_manager.try_modify_type(frame_index, FrameType::MMIO)
            {
                panic!(
                    "Attempted to assign MMIO to Frame {}: {:?} into {:?}",
                    frame_index, from, to
                );
            }
        }

        let page_manager = global_pgmr();
        let ptr = frame_manager
            .phys_mem_offset()
            .as_mut_ptr::<u8>()
            .add(frame_index * 0x1000) as *mut u8;

        for offset in 0..count {
            page_manager.set_page_attribs(
                &Page::from_ptr(ptr.add(offset * 0x1000)),
                PageAttributes::UNCACHEABLE,
                AttributeModify::Insert,
            )
        }

        Self {
            ptr,
            len: count * 0x1000,
        }
    }

    pub fn mapped_addr(&self) -> crate::Address<crate::Virtual> {
        crate::Address::<crate::Virtual>::from_ptr(self.ptr)
    }

    pub fn pages(&self) -> core::ops::Range<Page> {
        let base_page = paging::Page::from_index((self.ptr as usize) / 0x1000);
        base_page
            ..(base_page
                .forward_checked(crate::align_up_div(self.len, 0x1000))
                .unwrap())
    }

    #[inline]
    const fn offset<T>(&self, offset: usize) -> *mut T {
        if (offset + core::mem::size_of::<T>()) < self.len {
            let ptr = unsafe { self.ptr.add(offset).cast::<T>() };

            if ptr.align_offset(core::mem::align_of::<T>()) == 0 {
                return ptr;
            }
        }

        core::ptr::null_mut()
    }

    #[inline]
    pub fn read<T>(&self, offset: usize) -> core::mem::MaybeUninit<T> {
        unsafe {
            self.offset::<core::mem::MaybeUninit<T>>(offset)
                .read_volatile()
        }
    }

    #[inline]
    pub fn write<T>(&self, offset: usize, value: T) {
        unsafe { self.offset::<T>(offset).write_volatile(value) }
    }

    #[inline(always)]
    pub unsafe fn read_unchecked<T>(&self, offset: usize) -> T {
        core::ptr::read_volatile(self.ptr.add(offset) as *const T)
    }

    #[inline(always)]
    pub unsafe fn write_unchecked<T>(&self, offset: usize, value: T) {
        core::ptr::write_volatile(self.ptr.add(offset) as *mut T, value);
    }

    #[inline]
    pub const unsafe fn borrow<T: volatile::Volatile>(&self, offset: usize) -> &T {
        self.offset::<T>(offset).as_ref().unwrap()
    }

    #[inline]
    pub const unsafe fn slice<'a, T: volatile::Volatile>(
        &'a self,
        offset: usize,
        len: usize,
    ) -> Option<&'a [T]> {
        if (offset + (len * core::mem::size_of::<T>())) < self.len {
            Some(core::slice::from_raw_parts(self.offset::<T>(offset), len))
        } else {
            None
        }
    }
}

impl core::fmt::Debug for MMIO {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MMIO")
            .field("Virtual Base", &self.ptr)
            .field("Length", &self.len)
            .finish()
    }
}
