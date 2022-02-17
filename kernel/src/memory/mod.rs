unsafe fn init_memory() {
    use libkernel::memory::Page;

    // Set kernel page
    KERNEL_PAGE_MANAGER
        .set(PageManager::new(&Page::null()))
        .unwrap_or_else(|_| panic!(""));
    libkernel::memory::set_page_manager(KERNEL_PAGE_MANAGER.get().unwrap_or_else(|| panic!("")));
    // Set kernel mallocator.
    KERNEL_MALLOCATOR
        .set(slob::SLOB::new())
        .unwrap_or_else(|_| panic!(""));

    // Configure and use page manager.
    {
        use libkernel::memory::{FrameType, FRAME_MANAGER};
        info!("Initializing kernel SLOB allocator.");

        {
            // TODO abstract this into the kernel itself
            let page_manager = libkernel::memory::get_page_manager();

            use libkernel::memory::PageAttributes;

            // Set page attributes for UEFI descriptor pages.
            for descriptor in libkernel::BOOT_INFO.get().unwrap().memory_map().iter() {
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
                        page_manager
                            .identity_map(
                                &page,
                                PageAttributes::PRESENT | PageAttributes::GLOBAL | page_attribs,
                            )
                            .unwrap();
                    }
                }
            }

            // Overwrite UEFI page attributes for kernel ELF sections.
            use libkernel::{align_down_div, align_up_div};
            let kernel_text = Page::range(
                align_down_div(__text_start.as_usize(), 0x1000),
                align_up_div(__text_end.as_usize(), 0x1000),
            );
            let kernel_rodata = Page::range(
                align_down_div(__rodata_start.as_usize(), 0x1000),
                align_up_div(__rodata_end.as_usize(), 0x1000),
            );
            let kernel_data = Page::range(
                align_down_div(__data_start.as_usize(), 0x1000),
                align_up_div(__data_end.as_usize(), 0x1000),
            );
            let kernel_bss = Page::range(
                align_down_div(__bss_start.as_usize(), 0x1000),
                align_up_div(__bss_end.as_usize(), 0x1000),
            );
            let ap_text = Page::range(
                align_down_div(__ap_text_start.as_usize(), 0x1000),
                align_up_div(__ap_text_end.as_usize(), 0x1000),
            );
            let ap_data = Page::range(
                align_down_div(__ap_data_start.as_usize(), 0x1000),
                align_up_div(__ap_data_end.as_usize(), 0x1000),
            );
            let user_code = Page::range(
                align_down_div(__user_code_start.as_usize(), 0x1000),
                align_up_div(__user_code_end.as_usize(), 0x1000),
            );

            for page in kernel_text.chain(ap_text) {
                page_manager
                    .identity_map(&page, PageAttributes::PRESENT | PageAttributes::GLOBAL)
                    .unwrap();
            }

            for page in kernel_rodata {
                page_manager
                    .identity_map(
                        &page,
                        PageAttributes::PRESENT
                            | PageAttributes::GLOBAL
                            | PageAttributes::NO_EXECUTE,
                    )
                    .unwrap();
            }

            for page in kernel_data.chain(kernel_bss).chain(ap_data).chain(
                // Frame manager map frames/pages.
                FRAME_MANAGER
                    .iter()
                    .enumerate()
                    .filter_map(|(frame_index, (ty, _, _))| {
                        if ty == FrameType::FrameMap {
                            Some(Page::from_index(frame_index))
                        } else {
                            None
                        }
                    }),
            ) {
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

            for page in user_code {
                page_manager
                    .identity_map(&page, PageAttributes::PRESENT | PageAttributes::USERSPACE)
                    .unwrap();
            }

            // Since we're using physical offset mapping for our page table modification
            //  strategy, the memory needs to be identity mapped at the correct offset.
            let phys_mapping_addr = libkernel::memory::virtual_map_offset();
            debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
            page_manager.modify_mapped_page(Page::from_addr(phys_mapping_addr));

            info!("Writing kernel addressor's PML4 to the CR3 register.");
            page_manager.write_cr3();
        }

        // Configure SLOB allocator.
        debug!("Allocating reserved physical memory frames...");
        let slob = KERNEL_MALLOCATOR.get().unwrap();
        FRAME_MANAGER
            .iter()
            .enumerate()
            .filter(|(_, (ty, _, _))| !matches!(ty, FrameType::Usable))
            .for_each(|(index, _)| {
                slob.reserve_page(&Page::from_index(index)).unwrap();
            });

        info!("Finished block allocator initialization.");
    }

    debug!("Setting newly-configured default allocator.");
    libkernel::memory::malloc::set(KERNEL_MALLOCATOR.get().unwrap());
    // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
    debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
    __kernel_pml4
        .as_mut_ptr::<u32>()
        .write(libkernel::registers::control::CR3::read().0.as_usize() as u32);

    info!("Kernel memory initialized.");
}