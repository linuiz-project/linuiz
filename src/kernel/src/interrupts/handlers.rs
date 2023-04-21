use crate::{
    interrupts::Vector,
    proc::{ElfData, Registers, State},
};
use libsys::{page_size, Address, Page, Virtual};

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
        info!("{:?}", process.id());
        let elf_vaddr = process.load_address_to_elf_vaddr(address).unwrap();
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

        // Map the page as RW so we can copy the ELF data in.
        let mapped_memory = process
            .address_space_mut()
            .mmap(
                Some(Address::<Page>::new_truncate(address.get())),
                core::num::NonZeroUsize::MIN,
                crate::proc::MmapPermissions::ReadWrite,
            )
            .unwrap();

        // Calculate the range of bytes we will be reading from the ELF file.
        let file_offset = usize::try_from(phdr.p_offset).unwrap();
        let file_slice_len = usize::min(usize::try_from(phdr.p_filesz).unwrap(), page_size());
        // Subslice the ELF memory to get the requisite segment data.
        let file_slice = match process.elf_data() {
            ElfData::Memory(elf_memory) => &elf_memory[file_offset..(file_offset + file_slice_len)],
            ElfData::File(_) => unimplemented!(),
        };

        // Load the ELF data.
        let mapped_memory = mapped_memory.as_uninit_slice_mut();
        // Front padding is all of the bytes before the file offset.
        let (front_pad, mapped_memory) = mapped_memory.split_at_mut(file_offset % mapped_memory.len());
        // End padding is all of the bytes after the file offset + file slice length.
        let (mapped_memory, end_pad) = mapped_memory.split_at_mut(file_slice.len());
        // Zero the padding bytes, according to ELF spec.
        front_pad.fill(core::mem::MaybeUninit::new(0x0));
        end_pad.fill(core::mem::MaybeUninit::new(0x0));
        // Copy the ELF data into memory
        // Safety: In-place cast to a transparently aligned type.
        mapped_memory.copy_from_slice(unsafe { file_slice.align_to().1 });

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
