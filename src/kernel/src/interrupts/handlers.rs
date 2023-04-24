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
        use libsys::page_size;

        let process = scheduler.process_mut().ok_or(PageFaultHandlerError)?;
        let fault_elf_vaddr = address.get() - process.load_offset();
        let phdr = process
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| {
                (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz)).contains(&u64::try_from(fault_elf_vaddr).unwrap())
            })
            .ok_or(PageFaultHandlerError)?
            .clone();

        // Small check to help ensure the phdr alignments are page-fit.
        debug_assert_eq!(phdr.p_align & (libsys::page_mask() as u64), 0);

        let fault_page = Address::<Page>::new_truncate(address.get());
        trace!("Demand mapping {:?} from segment: {:?}", fault_page.as_ptr(), phdr);

        let fault_vaddr = fault_page.get().get() - process.load_offset();
        let segment_vaddr = usize::try_from(phdr.p_vaddr).unwrap();
        let segment_file_size = usize::try_from(phdr.p_vaddr).unwrap();

        let fault_segment_offset = fault_vaddr.checked_sub(segment_vaddr).unwrap_or(0);
        let segment_front_pad = segment_vaddr.checked_sub(fault_vaddr).unwrap_or(0);

        let fault_segment_range = fault_segment_offset..(fault_segment_offset + page_size());
        let padded_segment_offset = fault_segment_offset + segment_front_pad;

        let segment_file_end = padded_segment_offset + segment_file_size;
        let segment_range_end = usize::min(fault_segment_range.end, segment_file_end);
        let segment_range = padded_segment_offset..segment_range_end;

        // Map the page as RW so we can copy the ELF data in.
        let mapped_memory = process
            .address_space_mut()
            .mmap(Some(fault_page), core::num::NonZeroUsize::MIN, crate::proc::MmapPermissions::ReadWrite)
            .unwrap();
        let mapped_memory = mapped_memory.as_uninit_slice_mut();

        let (front_pad, remaining) = mapped_memory.split_at_mut(segment_front_pad);
        let (file_memory, end_pad) = remaining.split_at_mut(segment_range.len());
        front_pad.fill(core::mem::MaybeUninit::new(0x0));
        end_pad.fill(core::mem::MaybeUninit::new(0x0));

        if file_memory.len() > 0 {
            match process.elf_data() {
                ElfData::Memory(data) => {
                    file_memory.copy_from_slice(unsafe { data.get(segment_range).unwrap().align_to().1 });
                }
                ElfData::File(_) => unimplemented!(),
            }
        }

        // Process any relocations.
        let load_offset = process.load_offset();
        let fault_page_mem_range = fault_vaddr..(fault_vaddr + page_size());
        process.elf_relas().drain_filter(|rela| {
            if fault_page_mem_range.contains(&rela.address.get()) {
                trace!("Processing relocation: {:X?}", rela);
                rela.address.as_ptr().add(load_offset).cast::<usize>().write(rela.value);

                true
            } else {
                false
            }
        });

        process
            .address_space_mut()
            .set_flags(
                fault_page,
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
