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
        let process = scheduler.process_mut().ok_or(PageFaultHandlerError)?;
        let page_phdr = process
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz)).contains(&u64::try_from(address.get()).unwrap()))
            .ok_or(PageFaultHandlerError)?
            .clone();

        // Convert some relevant phdr fields to `usize` for working with.
        let vaddr = usize::try_from(page_phdr.p_vaddr).unwrap();
        let file_size = usize::try_from(page_phdr.p_filesz).unwrap();
        // Convert the virtual address to a page address.
        let page_address = Address::<Page>::new_truncate(address.get());
        let mem_offset = vaddr - page_address.get().get();
        // Calculate the portion of the memory range which resides within the ELF itself.
        let file_offset = usize::try_from(page_phdr.p_offset).unwrap() + mem_offset;
        let file_portion = file_offset..usize::min(vaddr + file_size, file_offset + page_size());

        // Map the page into memory.
        let mapped_memory = process
            .address_space_mut()
            .mmap(Some(page_address), core::num::NonZeroUsize::MIN, crate::proc::MmapPermissions::ReadWrite)
            .unwrap();

        // Find the ELF data.
        let elf_data = match process.elf_data() {
            ElfData::Memory(elf_memory) => &elf_memory[file_portion],
            ElfData::File(_) => unimplemented!(),
        };

        // Load the ELF data.
        let mapped_memory = mapped_memory.as_uninit_slice_mut();
        let (copy_memory, clear_memory) = mapped_memory.split_at_mut(elf_data.len());
        // Copy the data into memory.
        copy_memory.copy_from_slice(unsafe { elf_data.align_to().1 });
        // Zero the remaining bytes, according to ELF spec.
        clear_memory.fill(core::mem::MaybeUninit::new(0x0));

        use crate::memory::paging::TableEntryFlags;
        process
            .address_space_mut()
            .set_flags(
                page_address,
                core::num::NonZeroUsize::MIN,
                TableEntryFlags::PRESENT
                    | TableEntryFlags::USER
                    | TableEntryFlags::from(crate::proc::segment_type_to_mmap_permissions(page_phdr.p_type)),
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
