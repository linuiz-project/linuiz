use libsys::{Address, Page};
use try_alloc::boxed::TryBox;

pub fn load_modules() {
    let drivers_data = crate::boot::get_kernel_modules()
        // Find the drives module, and map the `Option<>` to it.
        .and_then(|modules| {
            modules.iter().find(|module| module.path.to_str().unwrap().to_str().unwrap().ends_with("drivers"))
        })
        // ### Safety: Kernel promises HHDM to be valid, and the module pointer should be in the HHDM, so this should be valid for `u8`.
        .map(|drivers_module| unsafe {
            core::slice::from_raw_parts(drivers_module.base.as_ptr().unwrap(), drivers_module.length as usize)
        })
        .expect("no drivers provided");

    for (header, data) in lza::ArchiveReader::new(drivers_data) {
        // SAFETY: Value is non-zero.
        let Ok(mut elf_buffer) = TryBox::new_slice(header.len().get(), 0u8)
            else {
                warn!("Failed allocate decompression buffer for driver: {:?}", header);
                continue
            };

        let mut inflate_state = miniz_oxide::inflate::stream::InflateState::new(miniz_oxide::DataFormat::Raw);
        let inflate_result = miniz_oxide::inflate::stream::inflate(
            &mut inflate_state,
            data,
            &mut *elf_buffer,
            miniz_oxide::MZFlush::Finish,
        );
        if inflate_result.status.is_err() {
            warn!(
                "Failed decompress driver blob:\n{:#?}\n{:#?}\nData Snippet: {:?}",
                header,
                inflate_result,
                &data[..100]
            );
            continue;
        };

        let Some(elf) = crate::elf::Elf::from_bytes(&*elf_buffer)
            else {
                warn!("Failed parse driver blob into valid ELF: {:?}", header);
                continue
            };

        info!("{:?}", elf);

        /* load driver */
        {
            use crate::{elf::segment, memory::PageAttributes};
            use libsys::PageAlign;

            // Create the driver's page manager from the kernel's higher-half table.
            // ### Safety: Kernel guarantees HHDM to be valid.
            let mut driver_page_manager = unsafe {
                crate::memory::address_space::Mapper::new(
                    4,
                    crate::memory::hhdm_address(),
                    Some(crate::memory::PagingRegister::read()),
                )
                .expect("failed to create page manager for driver")
            };

            let hhdm_address = crate::memory::hhdm_address();

            // Iterate the segments, and allocate them.
            for segment in elf.iter_segments() {
                trace!("{:?}", segment);

                match segment.get_type() {
                    segment::Type::Loadable => {
                        let memory_start = segment.get_virtual_address().unwrap().as_usize();
                        let memory_end = memory_start + segment.get_memory_layout().unwrap().size();
                        // ### Safety: Value provided is non-zero.
                        let start_page_index = libsys::align_down_div(memory_start, unsafe {
                            core::num::NonZeroUsize::new_unchecked(0x1000)
                        });
                        // ### Safety: Value provided is non-zero.
                        let end_page_index =
                            libsys::align_up_div(memory_end, unsafe { core::num::NonZeroUsize::new_unchecked(0x1000) });
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

                            let page =
                                Address::<Page>::from_u64((page_index * 0x1000) as u64, Some(PageAlign::Align4KiB))
                                    .unwrap();
                            driver_page_manager.auto_map(page, page_attributes | PageAttributes::USER).unwrap();

                            // ### Safety: HHDM is guaranteed by kernel to be valid, and the frame being pointed to was just allocated.
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
                // let stack_address = {
                //     const TASK_STACK_BASE_ADDRESS: Address<Page> = Address::<Page>::new_truncate(
                //         Address::<Virtual>::new_truncate(128 << 39),
                //         Some(PageAlign::Align2MiB),
                //     );
                //     // TODO make this a dynamic configuration
                //     const TASK_STACK_PAGE_COUNT: usize = 2;

                //     for page in (0..TASK_STACK_PAGE_COUNT)
                //         .map(|offset| TASK_STACK_BASE_ADDRESS.forward_checked(offset).unwrap())
                //     {
                //         driver_page_manager
                //             .map(
                //                 page,
                //                 Address::<Frame>::zero(),
                //                 false,
                //                 PageAttributes::WRITABLE
                //                     | PageAttributes::NO_EXECUTE
                //                     | PageAttributes::DEMAND
                //                     | PageAttributes::USER
                //                     | PageAttributes::HUGE,
                //             )
                //             .unwrap();
                //     }

                //     TASK_STACK_BASE_ADDRESS.forward_checked(TASK_STACK_PAGE_COUNT).unwrap()
                // };

                // TODO
                // let task = crate::local_state::Task::new(
                //     u8::MIN,
                //     // TODO account for memory base when passing entry offset
                //     crate::local_state::EntryPoint::Address(
                //         Address::<Virtual>::new(elf.get_entry_offset() as u64).unwrap(),
                //     ),
                //     stack_address.address(),
                //     {
                //         #[cfg(target_arch = "x86_64")]
                //         {
                //             (
                //                 crate::arch::x64::registers::GeneralRegisters::empty(),
                //                 crate::arch::x64::registers::SpecialRegisters::flags_with_user_segments(
                //                     crate::arch::x64::registers::RFlags::INTERRUPT_FLAG,
                //                 ),
                //             )
                //         }
                //     },
                //     #[cfg(target_arch = "x86_64")]
                //     {
                //         // TODO do not error here ?
                //         driver_page_manager.read_vmem_register().unwrap()
                //     },
                // );

                // crate::local_state::queue_task(task);
            }
        }
    }
}
