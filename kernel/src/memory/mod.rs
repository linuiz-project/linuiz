
// lazy_static::lazy_static! {
//     pub static ref KMALLOC: SLOB<'static> = SLOB::new();
// }

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
    //         .filter(|(_, (ty, _, _))| !matches!(ty, liblz::memory::FrameType::Usable))
    //         .for_each(|(index, _)| {
    //             KMALLOC.reserve_page(&Page::from_index(index)).unwrap();
    //         });

    //     info!("Finished block allocator initialization.");
    // }

    // debug!("Setting newly-configured default allocator.");
    // liblz::memory::malloc::set(&*KMALLOC);
    // // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
    // debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
    // __kernel_pml4
    //     .as_mut_ptr::<u32>()
    //     .write(liblz::registers::control::CR3::read().0.as_usize() as u32);

    // info!("Kernel memory initialized.");
