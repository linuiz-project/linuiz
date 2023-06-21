use libsys::{Address, Page, Virtual};

crate::error_impl! {
    /// Indicates what type of error the common page fault handler encountered.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Error {
        CoreState => None,
        NoTask => None,
        ElfData => None,
        UnhandledAddress { addr: Address<Virtual> } => None
    }
}

/// ### Safety
///
/// This function should only be called in the case of passing context to handle a page fault.
/// Calling this function more than once and/or outside the context of a page fault is undefined behaviour.
#[doc(hidden)]
#[inline(never)]
pub unsafe fn handler(fault_address: Address<Virtual>) -> Result<()> {
    crate::cpu::state::with_scheduler(|scheduler| {
        // TODO this code doesn't belong here

        use crate::{mem::paging::TableEntryFlags, task::ElfData};
        use core::mem::MaybeUninit;
        use libsys::page_size;

        let task = scheduler.task_mut().ok_or(Error::NoTask)?;

        let fault_unoffset = fault_address
            .get()
            .checked_sub(task.load_offset())
            .ok_or(Error::UnhandledAddress { addr: fault_address })?;

        let segment = *task
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| {
                (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz)).contains(&u64::try_from(fault_unoffset).unwrap())
            })
            .ok_or(Error::ElfData)?;

        // Small check to help ensure the phdr alignments are page-fit.
        debug_assert_eq!(segment.p_align & (libsys::page_mask() as u64), 0);

        let fault_page = Address::new_truncate(fault_address.get());

        let fault_unoffset_page: Address<Page> = Address::new_truncate(fault_unoffset);
        let fault_unoffset_page_addr = fault_unoffset_page.get().get();

        let fault_unoffset_end_page: Address<Page> = Address::new_truncate(fault_unoffset_page_addr + page_size());
        let fault_unoffset_end_page_addr = fault_unoffset_end_page.get().get();

        debug!("Demand mapping {:X?} from segment: {:X?}", Address::<Page>::new_truncate(fault_address.get()), segment);

        let segment_addr = usize::try_from(segment.p_vaddr).unwrap();
        let segment_size = usize::try_from(segment.p_filesz).unwrap();
        let segment_end_addr = segment_addr + segment_size;

        let fault_offset = fault_unoffset_page_addr.saturating_sub(segment_addr);
        let fault_end_pad = fault_unoffset_end_page_addr.saturating_sub(segment_end_addr);
        let fault_front_pad = segment_addr.saturating_sub(fault_unoffset_page_addr);
        let fault_size = ((fault_unoffset_end_page_addr - fault_unoffset_page_addr) - fault_front_pad) - fault_end_pad;

        trace!("Mapping the demand page RW so data can be copied.");
        let mapped_memory = task
            .address_space_mut()
            .mmap(Some(fault_page), core::num::NonZeroUsize::MIN, crate::task::MmapPermissions::ReadWrite)
            .unwrap()
            .as_uninit_slice_mut();

        let (front_pad, remaining) = mapped_memory.split_at_mut(fault_front_pad);
        let (file_memory, end_pad) = remaining.split_at_mut(fault_size);

        debug_assert_eq!(fault_front_pad, front_pad.len(), "front padding");
        debug_assert_eq!(fault_end_pad, end_pad.len(), "end padding");
        debug_assert_eq!(fault_size, file_memory.len(), "file memory");

        trace!(
            "Copying memory into demand mapping: {:#X}..{:#X}..{:#X}.",
            front_pad.len(),
            file_memory.len(),
            end_pad.len()
        );
        front_pad.fill(MaybeUninit::uninit());
        end_pad.fill(MaybeUninit::uninit());

        if !file_memory.is_empty() {
            match task.elf_data() {
                ElfData::Memory(data) => {
                    let segment_data_offset = usize::try_from(segment.p_offset).unwrap();

                    let offset_segment_range =
                        (segment_data_offset + fault_offset)..(segment_data_offset + fault_offset + fault_size);

                    // Safety: Same-sized reinterpret for copying.
                    let (_, copy_data, _) = unsafe { data[offset_segment_range].align_to() };

                    file_memory.copy_from_slice(copy_data);
                }
                ElfData::File(_) => unimplemented!(),
            }
        }

        // Safety: Slice has been initialized with values.
        let _mapped_memory = unsafe { MaybeUninit::slice_assume_init_mut(mapped_memory) };

        trace!("Processing demand mapping relocations.");
        let load_offset = task.load_offset();
        let fault_page_as_range = fault_unoffset_page_addr..fault_unoffset_end_page_addr;

        task.elf_relas().retain(|rela| {
            if fault_page_as_range.contains(&rela.address.get()) {
                trace!("Processing relocation: {:X?}", rela);
                rela.address.as_ptr().add(load_offset).cast::<usize>().write(rela.value);

                false
            } else {
                true
            }
        });

        trace!("Properly calculating page's attributes.");
        task.address_space_mut()
            .set_flags(
                fault_page,
                core::num::NonZeroUsize::new(1).unwrap(),
                TableEntryFlags::PRESENT
                    | TableEntryFlags::USER
                    | TableEntryFlags::from(crate::task::segment_to_mmap_permissions(segment.p_type)),
            )
            .unwrap();

        trace!("Demand mapping complete.");

        Ok(())
    })
}
