use try_alloc::vec::TryVec;

pub fn load() -> TryVec<&'static str> {
    #[limine::limine_tag]
    static LIMINE_MODULES: limine::ModuleRequest = limine::ModuleRequest::new(crate::boot::LIMINE_REV);

    let mut loaded_modules = TryVec::new();

    let Some(modules) = LIMINE_MODULES.get_response() else { return loaded_modules };

    for archive_entry in modules
        .modules()
        .iter()
        // Filter out modules that don't end with our driver postfix.
        .filter(|module| module.path().ends_with("drivers"))
        // Flat map the tar archive entries into an iterator.
        .flat_map(|module| tar_no_std::TarArchiveRef::new(module.data()).entries())
    {
        use crate::memory::PageDepth;
        use elf::{endian::AnyEndian, ElfBytes};
        use libsys::{page_shift, page_size, Address};

        let archive_entry_filename = archive_entry.filename();

        debug!("Processing archive entry for driver: {}", archive_entry_filename);
        let Ok(driver_elf) = ElfBytes::<AnyEndian>::minimal_parse(archive_entry.data())
        else {
            warn!("failed to parse driver blob into valid ELF.");
            continue
        };

        // Create the driver's page manager from the kernel's higher-half table.
        // Safety: Kernel guarantees HHDM to be valid.
        let mut driver_mapper = unsafe {
            crate::memory::address_space::mapper::Mapper::new_unsafe(
                PageDepth::new(4),
                crate::memory::new_kmapped_page_table().unwrap(),
            )
        };

        // Iterate the segments, and allocate them.
        let Some(segments) = driver_elf.segments() else { continue };

        // Parse loadable segments.
        segments.iter().filter(|phdr| phdr.p_type == elf::abi::PT_LOAD).for_each(|phdr| {
            trace!("{:?}", phdr);

            let memory_size = usize::try_from(phdr.p_memsz).unwrap();
            let memory_start = usize::try_from(phdr.p_vaddr).unwrap();
            let memory_end = memory_start + memory_size;

            // Align the start address to ensure we iterate page-aligned addresses.
            let memory_start_aligned = libsys::align_down(memory_start, page_shift());
            for page_base in (memory_start_aligned..memory_end).step_by(page_size()) {
                use bit_field::BitField;

                // Auto map the virtual address to a physical page.
                let page = Address::new(page_base).unwrap();
                //trace!("{:?} auto map {:X?}", name, page);
                driver_mapper
                    .auto_map(page, {
                        use crate::memory::paging::Attributes;

                        // This doesn't support RWX pages. I'm not sure it ever should.
                        if phdr.p_flags.get_bit(1) {
                            Attributes::RX
                        } else if phdr.p_flags.get_bit(2) {
                            Attributes::RW
                        } else {
                            Attributes::RO
                        }
                    })
                    .unwrap();
            }

            let segment_slice = driver_elf.segment_data(&phdr).expect("driver segment parse failure");
            // Safety: `memory_start` pointer is valid as we just mapped all of the requisite pages for `memory_size` length.
            let memory_slice = unsafe { core::slice::from_raw_parts_mut(memory_start as *mut u8, memory_size) };
            // Copy segment data into the new memory region.
            memory_slice[..segment_slice.len()].copy_from_slice(segment_slice);
            // Clear any left over bytes to 0. This is useful for the bss region, for example.
            memory_slice[segment_slice.len()..].fill(0x0);

            loaded_modules.push(&archive_entry_filename).unwrap();
        });
    }

    loaded_modules
}
