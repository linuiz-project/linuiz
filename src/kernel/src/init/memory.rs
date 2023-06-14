use crate::mem::paging::{self, TableDepth};
use core::ops::Range;
use libsys::{page_size, Address};

crate::error_impl! {
    #[derive(Debug)]
    enum Error {
        KernelAddress => None,
        Paging { err: paging::Error } => Some(err)
    }
}

struct KernelAddresses {
    phys: usize,
    virt: usize,
}

fn get_kernel_addresses() -> Result<KernelAddresses> {
    #[limine::limine_tag]
    static LIMINE_KERNEL_ADDR: limine::KernelAddressRequest =
        limine::KernelAddressRequest::new(crate::boot::LIMINE_REV);

    LIMINE_KERNEL_ADDR
        .get_response()
        .map(|response| KernelAddresses {
            phys: usize::try_from(response.physical_base()).unwrap(),
            virt: usize::try_from(response.virtual_base()).unwrap(),
        })
        .ok_or(Error::KernelAddress)
}

#[allow(clippy::too_many_lines)]
pub fn setup() {
    // Extract kernel address information.
    let kernel_addresses = get_kernel_addresses().expect("bootloader did not provide kernel address info");

    debug!("Preparing kernel memory system.");

    // Take reference to kernel file data.
    let kernel_file = crate::boot::kernel_file().expect("bootloader provided no kernel file");

    // Safety: Bootloader guarantees the provided information to be correct.
    let kernel_elf = elf::ElfBytes::<elf::endian::AnyEndian>::minimal_parse(kernel_file.data())
        .expect("kernel file is not a valid ELF");

    /* load and map segments */

    crate::mem::with_kmapper(|kmapper| {
        use crate::mem::{paging::TableEntryFlags, HHDM};
        use limine::MemoryMapEntryType;

        debug!("Mapping the higher-half direct map.");
        crate::boot::get_memory_map()
            .expect("bootloader memory map is required to map HHDM")
            .iter()
            .map(|entry| {
                let range = entry.range();
                (usize::try_from(range.start).unwrap()..usize::try_from(range.end).unwrap(), entry.ty())
            })
            .enumerate()
            .cycle()
            .try_reduce(|(last_index, (last_range, last_ty)), (index, (range, ty))| {
                use core::ops::ControlFlow;

                if last_range.end == range.start && last_ty == ty {
                    ControlFlow::Continue((index, (last_range.start..range.end, last_ty)))
                } else {
                    fn map_hhdm_range(
                        mapper: &mut crate::mem::mapper::Mapper,
                        mut range: Range<usize>,
                        flags: TableEntryFlags,
                        lock_frames: bool,
                    ) {
                        const HUGE_PAGE_DEPTH: TableDepth = TableDepth::new_mappable::<1>().unwrap();

                        trace!("HHDM Map  {:#X?}  {:?}   lock {}", range, flags, lock_frames);

                        while !range.is_empty() {
                            if range.len() > HUGE_PAGE_DEPTH.align()
                                && range.start.trailing_zeros() >= HUGE_PAGE_DEPTH.align().trailing_zeros()
                            {
                                let frame = Address::new(range.start).unwrap();
                                let page = HHDM.offset(frame).unwrap();
                                range.advance_by(HUGE_PAGE_DEPTH.align()).unwrap();

                                mapper
                                    .map(page, HUGE_PAGE_DEPTH, frame, lock_frames, flags | TableEntryFlags::HUGE)
                                    .expect("failed multi-page HHDM mapping");
                            } else {
                                let frame = Address::new(range.start).unwrap();
                                let page = HHDM.offset(frame).unwrap();
                                range.advance_by(page_size()).unwrap();

                                mapper
                                    .map(page, TableDepth::min(), frame, lock_frames, flags)
                                    .expect("failed single page HHDM mapping");
                            }
                        }
                    }

                    if last_range.end < range.start {
                        let tween_range = last_range.end..range.start;
                        map_hhdm_range(kmapper, tween_range, TableEntryFlags::RO, true);
                    }

                    let mmap_args = {
                        match last_ty {
                            MemoryMapEntryType::Usable => Some((TableEntryFlags::RW, false)),

                            MemoryMapEntryType::AcpiNvs
                            | MemoryMapEntryType::AcpiReclaimable
                            | MemoryMapEntryType::BootloaderReclaimable
                            | MemoryMapEntryType::Framebuffer => Some((TableEntryFlags::RW, true)),

                            MemoryMapEntryType::Reserved | MemoryMapEntryType::KernelAndModules => {
                                Some((TableEntryFlags::RO, true))
                            }

                            MemoryMapEntryType::BadMemory => None,
                        }
                    };

                    if let Some((flags, lock_frames)) = mmap_args {
                        map_hhdm_range(kmapper, last_range, flags, lock_frames);
                    } else {
                        trace!("HHDM Map (!! bad memory !!) @{:#X?}", last_range);
                    }

                    if last_index < index {
                        ControlFlow::Continue((index, (range, ty)))
                    } else {
                        ControlFlow::Break(())
                    }
                }
            });

        /* load kernel segments */
        kernel_elf
            .segments()
            .expect("kernel file has no segments")
            .into_iter()
            .filter(|ph| ph.p_type == elf::abi::PT_LOAD)
            .for_each(|phdr| {
                extern "C" {
                    static KERNEL_BASE: libkernel::LinkerSymbol;
                }

                debug!("{:X?}", phdr);

                // Safety: `KERNEL_BASE` is a linker symbol to an in-executable memory location, so it is guaranteed to be valid (and is never written to).
                let base_offset = usize::try_from(phdr.p_vaddr).unwrap() - unsafe { KERNEL_BASE.as_usize() };
                let base_offset_end = base_offset + usize::try_from(phdr.p_memsz).unwrap();
                let flags = TableEntryFlags::from(crate::task::segment_type_to_mmap_permissions(phdr.p_flags));

                (base_offset..base_offset_end)
                    .step_by(page_size())
                    // Attempt to map the page to the frame.
                    .try_for_each(|offset| {
                        let phys_addr = Address::new(kernel_addresses.phys + offset).unwrap();
                        let virt_addr = Address::new(kernel_addresses.virt + offset).unwrap();

                        trace!("Map  {:X?} -> {:X?}   {:?}", virt_addr, phys_addr, flags);
                        kmapper.map(virt_addr, TableDepth::min(), phys_addr, true, flags)
                    })
                    .expect("failed to map kernel segments");
            });

        debug!("Switching to kernel page tables...");
        // Safety: Kernel mappings should be identical to the bootloader mappings.
        unsafe { kmapper.swap_into() };
        debug!("Kernel has finalized control of page tables.");
    });
}
