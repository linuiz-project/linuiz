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
pub unsafe fn handler(address: Address<Virtual>) -> Result<()> {
    crate::cpu::state::with_scheduler(|scheduler| {
        use crate::{mem::paging::TableEntryFlags, task::ElfData};
        use core::mem::MaybeUninit;
        use libsys::page_size;

        let task = scheduler.task_mut().ok_or(Error::NoTask)?;
        let fault_elf_vaddr =
            address.get().checked_sub(task.load_offset()).ok_or(Error::UnhandledAddress { addr: address })?;
        let phdr = *task
            .elf_segments()
            .iter()
            .filter(|phdr| phdr.p_type == elf::abi::PT_LOAD)
            .find(|phdr| {
                (phdr.p_vaddr..(phdr.p_vaddr + phdr.p_memsz)).contains(&u64::try_from(fault_elf_vaddr).unwrap())
            })
            .ok_or(Error::ElfData)?;

        // Small check to help ensure the phdr alignments are page-fit.
        debug_assert_eq!(phdr.p_align & (libsys::page_mask() as u64), 0);

        let fault_page = Address::<Page>::new_truncate(address.get());
        debug!("Demand mapping {:X?} from segment: {:X?}", fault_page.as_ptr(), phdr);

        trace!("Calculating addresses and offsets for mapping.");
        let fault_vaddr = fault_page.get().get() - task.load_offset();
        let segment_vaddr = usize::try_from(phdr.p_vaddr).unwrap();
        let segment_file_size = usize::try_from(phdr.p_filesz).unwrap();

        let fault_segment_offset = fault_vaddr.saturating_sub(segment_vaddr);
        let segment_front_pad = segment_vaddr.saturating_sub(fault_vaddr);

        let fault_segment_range = fault_segment_offset..(fault_segment_offset + page_size());
        let padded_segment_offset = fault_segment_offset + segment_front_pad;

        let segment_file_end = padded_segment_offset + segment_file_size;
        let segment_range_end = usize::min(fault_segment_range.end, segment_file_end);
        let segment_range = padded_segment_offset..segment_range_end;

        // Map the page as RW so we can copy the ELF data in.
        trace!("Mapping the demand page RW so data can be copied.");
        let mapped_memory = task
            .address_space_mut()
            .mmap(Some(fault_page), core::num::NonZeroUsize::MIN, crate::task::MmapPermissions::ReadWrite)
            .unwrap();
        let mapped_memory = mapped_memory.as_uninit_slice_mut();

        let (front_pad, remaining) = mapped_memory.split_at_mut(segment_front_pad);
        let (file_memory, end_pad) = remaining.split_at_mut(segment_range.len());

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
                    // Safety: Same-sized reinterpret for copying.
                    let (pre, copy_data, post) = unsafe { data.get(segment_range).unwrap().align_to() };
                    assert!(pre.is_empty());
                    assert!(post.is_empty());

                    file_memory.copy_from_slice(copy_data);
                }
                ElfData::File(_) => unimplemented!(),
            }
        }

        // Safety: Array has been initialized with values.
        let _mapped_memory = unsafe { MaybeUninit::slice_assume_init_mut(mapped_memory) };

        // Process any relocations.
        trace!("Processing demand mapping relocations.");
        let load_offset = task.load_offset();
        let fault_page_mem_range = fault_vaddr..(fault_vaddr + page_size());

        task.elf_relas().retain(|rela| {
            if fault_page_mem_range.contains(&rela.address.get()) {
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
                    | TableEntryFlags::from(crate::task::segment_to_mmap_permissions(phdr.p_type)),
            )
            .unwrap();

        trace!("Demand mapping complete.");

        Ok(())
    })
}
