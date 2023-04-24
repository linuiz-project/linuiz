use crate::{
    interrupts::Vector,
    proc::{ElfData, Registers, State},
};
use libsys::{Address, Page, Virtual};

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy)]
pub struct PageFaultHandlerError;

/// ### Safety
///
/// This function should only be called in the case of passing context to handle a page fault.
/// Calling this function more than once and/or outside the context of a page fault is undefined behaviour.
#[doc(hidden)]
#[repr(align(0x10))]
pub unsafe fn pf_handler(address: Address<Virtual>) -> Result<(), PageFaultHandlerError> {
    crate::local::with_scheduler(|scheduler| {
        use crate::memory::paging::TableEntryFlags;

        let process = scheduler.process_mut().ok_or(PageFaultHandlerError)?;
        let elf_vaddr = process
            .load_address_to_elf_vaddr(address)
            .unwrap_or_else(|| panic!("failed to calculate ELF address for page fault: {:X?}", address));
        let phdr = process
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz)).contains(&u64::try_from(elf_vaddr).unwrap()))
            .ok_or(PageFaultHandlerError)?
            .clone();

        // Small check to help ensure the phdr alignments are page-fit.
        debug_assert_eq!(phdr.p_align & (libsys::page_mask() as u64), 0);
        trace!("Demand mapping from segment: {:?}", phdr);

        let load_offset = process.load_offset();
        let segment_vaddr = usize::try_from(phdr.p_vaddr).unwrap();
        let segment_vaddr_aligned = libsys::align_down(segment_vaddr, libsys::page_shift());
        let segment_offset = usize::try_from(phdr.p_offset).unwrap();
        let segment_file_size = usize::try_from(phdr.p_filesz).unwrap();
        let segment_mem_size = usize::try_from(phdr.p_filesz).unwrap();
        let segment_file_end = segment_offset + segment_file_size;

        let segment_page = Address::new_truncate(load_offset + segment_vaddr);
        let segment_page_count = libsys::align_up_div(segment_mem_size, libsys::page_shift());

        // Map the page as RW so we can copy the ELF data in.
        let mapped_memory = process
            .address_space_mut()
            .mmap(
                Some(segment_page),
                core::num::NonZeroUsize::new(segment_page_count).unwrap(),
                crate::proc::MmapPermissions::ReadWrite,
            )
            .unwrap();
        let mapped_memory = mapped_memory.as_uninit_slice_mut();
        let memory_padding = segment_vaddr - segment_vaddr_aligned;

        if segment_file_size > 0 {
            let file_slice = match process.elf_data() {
                ElfData::Memory(elf_memory) => elf_memory,
                ElfData::File(_) => unimplemented!(),
            };
            let copy_file_slice = &file_slice[segment_offset..segment_file_end];

            let memory_copy_range = memory_padding..(memory_padding + copy_file_slice.len());
            mapped_memory[memory_copy_range].copy_from_slice(unsafe { copy_file_slice.align_to().1 });
        }

        if segment_mem_size > segment_file_size {
            mapped_memory[(memory_padding + segment_file_end)..].fill(core::mem::MaybeUninit::new(0x0));
        }

        // Process any relocations.
        let phdr_mem_range = segment_vaddr..(segment_vaddr + segment_mem_size);
        process.elf_relas().drain_filter(|rela| {
            if phdr_mem_range.contains(&rela.address.get()) {
                info!("Processing relocation: {:X?}", rela);
                rela.address.as_ptr().add(load_offset).cast::<usize>().write(rela.value);

                true
            } else {
                false
            }
        });

        process
            .address_space_mut()
            .set_flags(
                Address::<Page>::new_truncate(address.get()),
                core::num::NonZeroUsize::MIN,
                TableEntryFlags::PRESENT
                    | TableEntryFlags::USER
                    | TableEntryFlags::from(crate::proc::segment_type_to_mmap_permissions(phdr.p_type)),
            )
            .unwrap();

        Ok(())
    })
}

/// ### Safety
///
/// This function should only be called in the case of passing context to handle an interrupt.
/// Calling this function more than once and/or outside the context of an interrupt is undefined behaviour.
#[doc(hidden)]
#[repr(align(0x10))]
pub unsafe fn handle_irq(irq_vector: u64, state: &mut State, regs: &mut Registers) {
    match Vector::try_from(irq_vector) {
        Ok(Vector::Timer) => crate::local::with_scheduler(|scheduler| scheduler.next_task(state, regs)),

        Err(err) => panic!("Invalid interrupt vector: {:X?}", err),
        vector_result => unimplemented!("Unhandled interrupt: {:?}", vector_result),
    }

    #[cfg(target_arch = "x86_64")]
    crate::local::end_of_interrupt();
}
