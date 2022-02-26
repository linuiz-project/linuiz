use core::{mem::MaybeUninit, ops::Range};
use libkernel::{
    align_down_div, align_up_div,
    cell::{SyncCell, SyncOnceCell},
    memory::{FrameManager, Level4, Page, PageManager, PageTable},
    LinkerSymbol,
};

extern "C" {
    pub static __kernel_pml4: LinkerSymbol;

    pub static __ap_text_start: LinkerSymbol;
    pub static __ap_text_end: LinkerSymbol;

    pub static __ap_data_start: LinkerSymbol;
    pub static __ap_data_end: LinkerSymbol;

    pub static __text_start: LinkerSymbol;
    pub static __text_end: LinkerSymbol;

    pub static __rodata_start: LinkerSymbol;
    pub static __rodata_end: LinkerSymbol;

    pub static __data_start: LinkerSymbol;
    pub static __data_end: LinkerSymbol;

    pub static __bss_start: LinkerSymbol;
    pub static __bss_end: LinkerSymbol;

    pub static __user_code_start: LinkerSymbol;
    pub static __user_code_end: LinkerSymbol;
}

// lazy_static::lazy_static! {
//     pub static ref KMALLOC: SLOB<'static> = SLOB::new();
// }

static FRAME_MANAGER: SyncOnceCell<FrameManager> = SyncOnceCell::new();
pub fn get_frame_manager() -> &'static FrameManager<'static> {
    FRAME_MANAGER.get().unwrap()
}

pub fn init_frame_manager(memory_map: &[libkernel::memory::uefi::MemoryDescriptor]) {
    if let Err(_) = FRAME_MANAGER.set(FrameManager::from_mmap(memory_map)) {
        panic!("Failed to initialize frame manager: already exists");
    }
}

lazy_static::lazy_static! {
    static ref GLOBAL_PAGE_MANAGER: PageManager = unsafe {
        PageManager::new(&Page::null(), FRAME_MANAGER.get().unwrap(), None)
    };
}

/// Initialize kernel memory (frame manager, page manager, etc.)
pub unsafe fn init_paging(memory_map: &[libkernel::memory::uefi::MemoryDescriptor]) {
    use libkernel::memory::PageAttributes;

    // Set page attributes for UEFI descriptor pages.
    for descriptor in memory_map {
        let mut page_attribs = PageAttributes::empty();

        use libkernel::memory::uefi::{MemoryAttributes, MemoryType};

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
                GLOBAL_PAGE_MANAGER
                    .identity_map(
                        &page,
                        PageAttributes::PRESENT | PageAttributes::GLOBAL | page_attribs,
                    )
                    .unwrap();
            }
        }
    }

    debug!("Global mapping kernel ELF sections.");
    let kernel_text = unsafe {
        Page::range(
            align_down_div(__text_start.as_usize(), 0x1000),
            align_up_div(__text_end.as_usize(), 0x1000),
        )
    };

    let kernel_rodata = unsafe {
        Page::range(
            align_down_div(__rodata_start.as_usize(), 0x1000),
            align_up_div(__rodata_end.as_usize(), 0x1000),
        )
    };

    let kernel_data = unsafe {
        Page::range(
            align_down_div(__data_start.as_usize(), 0x1000),
            align_up_div(__data_end.as_usize(), 0x1000),
        )
    };

    let kernel_bss = unsafe {
        Page::range(
            align_down_div(__bss_start.as_usize(), 0x1000),
            align_up_div(__bss_end.as_usize(), 0x1000),
        )
    };

    let ap_text = unsafe {
        Page::range(
            align_down_div(__ap_text_start.as_usize(), 0x1000),
            align_up_div(__ap_text_end.as_usize(), 0x1000),
        )
    };

    let ap_data = unsafe {
        Page::range(
            align_down_div(__ap_data_start.as_usize(), 0x1000),
            align_up_div(__ap_data_end.as_usize(), 0x1000),
        )
    };

    let user_code = unsafe {
        Page::range(
            align_down_div(__user_code_start.as_usize(), 0x1000),
            align_up_div(__user_code_end.as_usize(), 0x1000),
        )
    };

    for page in kernel_text.chain(ap_text) {
        GLOBAL_PAGE_MANAGER
            .identity_map(&page, PageAttributes::PRESENT | PageAttributes::GLOBAL)
            .unwrap();
    }

    for page in kernel_rodata {
        GLOBAL_PAGE_MANAGER
            .identity_map(
                &page,
                PageAttributes::PRESENT | PageAttributes::GLOBAL | PageAttributes::NO_EXECUTE,
            )
            .unwrap();
    }

    for page in
        kernel_data.chain(kernel_bss).chain(ap_data).chain(
            // Frame manager map frames/pages.
            FRAME_MANAGER.get().unwrap().iter().enumerate().filter_map(
                |(frame_index, (ty, _, _))| {
                    if ty == libkernel::memory::FrameType::FrameMap {
                        Some(Page::from_index(frame_index))
                    } else {
                        None
                    }
                },
            ),
        )
    {
        GLOBAL_PAGE_MANAGER
            .identity_map(
                &page,
                PageAttributes::PRESENT
                    | PageAttributes::GLOBAL
                    | PageAttributes::NO_EXECUTE
                    | PageAttributes::WRITABLE,
            )
            .unwrap();
    }

    for page in user_code {
        GLOBAL_PAGE_MANAGER
            .identity_map(&page, PageAttributes::PRESENT | PageAttributes::USERSPACE)
            .unwrap();
    }

    // Since we're using physical offset mapping for our page table modification
    //  strategy, the memory needs to be identity mapped at the correct offset.
    // todo PASS FRAME_MANAGER IN PARAMS
    debug!(
        "Mapping physical memory: @{:?}",
        FRAME_MANAGER.get().unwrap().virtual_map_offset()
    );
    GLOBAL_PAGE_MANAGER.modify_mapped_page(Page::from_addr(
        FRAME_MANAGER.get().unwrap().virtual_map_offset(),
    ));

    //     GLOBAL_PAGE_MANAGER.set(page_manager).unwrap();
    //     info!("Writing kernel addressor's PML4 to the CR3 register.");
    //     .write_cr3();
    // }

    //     // Configure SLOB allocator.
    //     debug!("Allocating reserved physical memory frames...");
    //     FRAME_MANAGER
    //         .get()
    //         .unwrap()
    //         .iter()
    //         .enumerate()
    //         .filter(|(_, (ty, _, _))| !matches!(ty, libkernel::memory::FrameType::Usable))
    //         .for_each(|(index, _)| {
    //             KMALLOC.reserve_page(&Page::from_index(index)).unwrap();
    //         });

    //     info!("Finished block allocator initialization.");
    // }

    // debug!("Setting newly-configured default allocator.");
    // libkernel::memory::malloc::set(&*KMALLOC);
    // // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
    // debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
    // __kernel_pml4
    //     .as_mut_ptr::<u32>()
    //     .write(libkernel::registers::control::CR3::read().0.as_usize() as u32);

    // info!("Kernel memory initialized.");
}

pub fn global_mapped_page() -> Page {
    GLOBAL_PAGE_MANAGER.mapped_page()
}

pub fn copy_global_pml4() -> PageTable<Level4> {
    GLOBAL_PAGE_MANAGER.copy_pml4()
}

pub unsafe fn write_global_cr3() {
    GLOBAL_PAGE_MANAGER.write_cr3();
}
