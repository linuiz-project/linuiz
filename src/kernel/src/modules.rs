use libcommon::{Address, Frame, Page, Virtual};

static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(crate::LIMINE_REV);
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(crate::LIMINE_REV);

fn drivers() {
    let drivers_data = LIMINE_MODULES
        .get_response()
        .get()
        // Find the drives module, and map the `Option<>` to it.
        .and_then(|modules| {
            modules.modules().iter().find(|module| module.path.to_str().unwrap().to_str().unwrap().ends_with("drivers"))
        })
        // SAFETY: Kernel promises HHDM to be valid, and the module pointer should be in the HHDM, so this should be valid for `u8`.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");

    let mut current_offset = 0;
    while current_offset < drivers_data.len() {
        // Copy and reconstruct the driver byte length from the prefix.
        let driver_len = {
            let mut value = 0;

            value |= (drivers_data[current_offset + 0] as u64) << 0;
            value |= (drivers_data[current_offset + 1] as u64) << 8;
            value |= (drivers_data[current_offset + 2] as u64) << 16;
            value |= (drivers_data[current_offset + 3] as u64) << 24;
            value |= (drivers_data[current_offset + 4] as u64) << 32;
            value |= (drivers_data[current_offset + 5] as u64) << 40;
            value |= (drivers_data[current_offset + 6] as u64) << 48;
            value |= (drivers_data[current_offset + 7] as u64) << 56;

            value as usize
        };

        let base_offset = current_offset + 8 /* skip 'len' prefix */;
        let driver_data =
            miniz_oxide::inflate::core::decompress(&drivers_data[base_offset..(base_offset + driver_len)])
                .expect("failed to decompress driver");
        let driver_elf = crate::elf::Elf::from_bytes(&driver_data).unwrap();
        info!("{:?}", driver_elf);

        /* load driver */
        {
            use crate::{elf::segment, memory::PageAttributes};
            use libcommon::PageAlign;

            // Create the driver's page manager from the kernel's higher-half table.
            // SAFETY: Kernel guarantees HHDM to be valid.
            let driver_page_manager = unsafe {
                crate::memory::Mapper::new(
                    4,
                    crate::memory::get_hhdm_address(),
                    Some(crate::memory::VmemRegister::read()),
                )
                .expect("failed to create page manager for driver")
            };

            let hhdm_address = crate::memory::get_hhdm_address();

            // Iterate the segments, and allocate them.
            for segment in driver_elf.iter_segments() {
                trace!("{:?}", segment);

                match segment.get_type() {
                    segment::Type::Loadable => {
                        let memory_start = segment.get_virtual_address().unwrap().as_usize();
                        let memory_end = memory_start + segment.get_memory_layout().unwrap().size();
                        // SAFETY: Value provided is non-zero.
                        let start_page_index = libcommon::align_down_div(memory_start, unsafe {
                            core::num::NonZeroUsize::new_unchecked(0x1000)
                        });
                        // SAFETY: Value provided is non-zero.
                        let end_page_index = libcommon::align_up_div(memory_end, unsafe {
                            core::num::NonZeroUsize::new_unchecked(0x1000)
                        });
                        let mut data_offset = 0;

                        for page_index in start_page_index..end_page_index {
                            // REMARK: This doesn't support RWX pages. I'm not sure it ever should.
                            let page_attributes = if segment.get_flags().contains(segment::Flags::EXECUTABLE) {
                                PageAttributes::RX
                            } else if segment.get_flags().contains(segment::Flags::WRITABLE) {
                                PageAttributes::RW
                            } else {
                                PageAttributes::RO
                            };

                            let page = Address::<Page>::new(
                                Address::<Virtual>::new((page_index * 0x1000) as u64).unwrap(),
                                Some(PageAlign::Align4KiB),
                            )
                            .unwrap();
                            driver_page_manager.auto_map(page, page_attributes | PageAttributes::USER).unwrap();

                            // SAFETY: HHDM is guaranteed by kernel to be valid, and the frame being pointed to was just allocated.
                            let memory_hhdm = unsafe {
                                core::slice::from_raw_parts_mut(
                                    hhdm_address
                                        .as_mut_ptr::<u8>()
                                        .add(driver_page_manager.get_mapped_to(page).unwrap().as_usize()),
                                    0x1000,
                                )
                            };

                            // If the virtual address isn't page-aligned, then this allows us to start writing at
                            // the correct address, rather than writing the wrong bytes at the lower page boundary.
                            let memory_offset = memory_start.checked_sub(page_index * 0x1000).unwrap_or(0);
                            // REMARK: This could likely be optimized to use memcpy / copy_nonoverlapping, but
                            //         for now this approach suffices.
                            for index in memory_offset..0x1000 {
                                let data_value = segment.data().get(data_offset);
                                memory_hhdm[index] = *data_value
                                    // Handle zeroing of `.bss` segments.
                                    .unwrap_or(&0);
                                data_offset += 1;
                            }
                        }
                    }

                    _ => {}
                }
            }

            // Push ELF as global task.
            {
                let stack_address = {
                    const TASK_STACK_BASE_ADDRESS: Address<Page> = Address::<Page>::new_truncate(
                        Address::<Virtual>::new_truncate(128 << 39),
                        PageAlign::Align2MiB,
                    );
                    // TODO make this a dynamic configuration
                    const TASK_STACK_PAGE_COUNT: usize = 2;

                    for page in (0..TASK_STACK_PAGE_COUNT)
                        .map(|offset| TASK_STACK_BASE_ADDRESS.forward_checked(offset).unwrap())
                    {
                        driver_page_manager
                            .map(
                                page,
                                Address::<Frame>::zero(),
                                false,
                                PageAttributes::WRITABLE
                                    | PageAttributes::NO_EXECUTE
                                    | PageAttributes::DEMAND
                                    | PageAttributes::USER
                                    | PageAttributes::HUGE,
                            )
                            .unwrap();
                    }

                    TASK_STACK_BASE_ADDRESS.forward_checked(TASK_STACK_PAGE_COUNT).unwrap()
                };

                let mut global_tasks = crate::local_state::GLOBAL_TASKS.lock();
                global_tasks.push_back(crate::local_state::Task::new(
                    crate::local_state::TaskPriority::new(crate::local_state::TaskPriority::MAX).unwrap(),
                    // TODO account for memory base when passing entry offset
                    crate::local_state::TaskStart::Address(
                        Address::<Virtual>::new(driver_elf.get_entry_offset() as u64).unwrap(),
                    ),
                    crate::local_state::TaskStack::At(stack_address.address()),
                    {
                        #[cfg(target_arch = "x86_64")]
                        {
                            (
                                crate::arch::x64::registers::GeneralRegisters::empty(),
                                crate::arch::x64::registers::SpecialRegisters::flags_with_user_segments(
                                    crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
                                ),
                            )
                        }
                    },
                    #[cfg(target_arch = "x86_64")]
                    {
                        // TODO do not error here ?
                        driver_page_manager.read_vmem_register().unwrap()
                    },
                ))
            }
        }

        current_offset += driver_len + 8  /* skip 'len' prefix */;
    }
}
