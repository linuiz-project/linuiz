use crate::memory::{
    allocators::global_memory_mut,
    paging::{Level4, PageAttributes, PageTable},
    Frame, Page,
};
use x86_64::{PhysAddr, VirtAddr};

pub trait VirtualAddessor {
    fn map(&mut self, page: &Page, frame: &Frame);
    fn unmap(&mut self, page: &Page);
    fn identity_map(&mut self, frame: &Frame);
    fn swap_into(&self);
}

pub struct MappedVirtualAddessor {
    mapped_addr: VirtAddr,
    pml4_frame: Frame,
}

impl MappedVirtualAddessor {
    /// Attempts to create a new MappedVirtualAddessor, with `current_mapped_addr` specifying the current virtual
    /// address where the entirety of the system physical memory is mapped.
    ///
    /// Safety: this method is unsafe because `current_mapped_addr` can be any value; that is, not necessarily
    /// a valid address in which physical memory is already mapped.
    pub unsafe fn new(current_mapped_addr: VirtAddr) -> Self {
        debug!(
            "Attempting to create a MappedVirtualAddessor (current mapped address supplied: {:?}).",
            current_mapped_addr
        );

        Self {
            // we don't know where physical memory is mapped at this point,
            // so rely on what the caller specifies for us
            mapped_addr: current_mapped_addr,
            pml4_frame: global_memory_mut(|allocator| {
                allocator
                    .lock_next()
                    .expect("failed to lock frame for PML4 of MappedVirtualAddessor")
            }),
        }
    }

    fn pml4_mut(&mut self) -> &mut PageTable<Level4> {
        unsafe {
            &mut *self
                .mapped_addr
                .as_mut_ptr::<PageTable<Level4>>()
                .offset(self.pml4_frame.index() as isize)
        }
    }

    pub fn modify_mapped_addr(&mut self, new_mapped_addr: VirtAddr) {
        debug!(
            "Modifying mapped offset: from {:?} to {:?}",
            self.mapped_addr, new_mapped_addr
        );
        debug!(
            "Mapping physical memory at offset address: {:?}",
            new_mapped_addr
        );

        let total_memory = global_memory_mut(|allocator| allocator.total_memory());
        for addr in (0..(total_memory as u64)).step_by(0x1000) {
            let virt_addr = VirtAddr::new(new_mapped_addr.as_u64() + addr);
            let phys_addr = PhysAddr::new(addr);
            self.map(&Page::from_addr(virt_addr), &Frame::from_addr(phys_addr));
        }

        self.mapped_addr = new_mapped_addr;
    }
}

impl VirtualAddessor for MappedVirtualAddessor {
    fn map(&mut self, page: &Page, frame: &Frame) {
        trace!("Mapping: {:?} to {:?}", page, frame);
        let offset = self.mapped_addr;
        let addr = (page.addr().as_u64() >> 12) as usize;
        let entry = &mut self
            .pml4_mut()
            .sub_table_create((addr >> 27) & 0x1FF, offset)
            .sub_table_create((addr >> 18) & 0x1FF, offset)
            .sub_table_create((addr >> 9) & 0x1FF, offset)[(addr >> 0) & 0x1FF];
        if entry.is_present() {
            // we invalidate entry in the TLB to ensure it isn't incorrectly followed
            crate::instructions::tlb::invalidate(page);
        }

        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", page, entry.frame(), entry);
    }

    fn unmap(&mut self, page: &Page) {
        let offset = self.mapped_addr;
        let addr = (page.addr().as_u64() >> 12) as usize;
        let entry = &mut self
            .pml4_mut()
            .sub_table_create((addr >> 27) & 0x1FF, offset)
            .sub_table_create((addr >> 18) & 0x1FF, offset)
            .sub_table_create((addr >> 9) & 0x1FF, offset)[(addr >> 0) & 0x1FF];

        entry.set_nonpresent();
        trace!("Unmapped {:?} from {:?}: {:?}", page, entry.frame(), entry);
    }

    fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    fn swap_into(&self) {
        info!(
            "Writing virtual addessor's PML4 to CR3 register: {:?}.",
            &self.pml4_frame
        );

        unsafe { crate::registers::CR3::write(&self.pml4_frame, None) };
    }
}
