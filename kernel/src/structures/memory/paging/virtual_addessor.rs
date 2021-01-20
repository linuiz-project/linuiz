use crate::structures::memory::{
    global_allocator, global_allocator_mut,
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
    pml4_ptr: *mut PageTable<Level4>,
}

impl MappedVirtualAddessor {
    pub fn new() -> Self {
        let (total_memory, pml4_frame) = global_allocator_mut(|allocator| {
            (
                allocator.total_memory(),
                allocator
                    .lock_next()
                    .expect("failed to lock frame for PML4 of MappedVirtualAddessor"),
            )
        });

        let mapped_addr = VirtAddr::new(0xFFFFFFFFFFFF - (total_memory as u64));
        Self {
            mapped_addr,
            pml4_ptr: unsafe {
                mapped_addr
                    .as_mut_ptr::<PageTable<Level4>>()
                    .offset(pml4_frame.index() as isize)
            },
        }
    }

    fn pml4(&self) -> &PageTable<Level4> {
        unsafe { &*self.pml4_ptr }
    }

    fn pml4_mut(&mut self) -> &mut PageTable<Level4> {
        unsafe { &mut *self.pml4_ptr }
    }

    fn pml4_frame(&self) -> Frame {
        Frame::from_addr(PhysAddr::new(
            (self.pml4_ptr as u64) - self.mapped_addr.as_u64(),
        ))
    }

    pub fn identity_map_full(&mut self) {
        debug!("Identity mapping all available memory.");
        let total_memory = global_allocator(|allocator| allocator.total_memory());
        for addr in (0..total_memory).step_by(0x1000) {
            self.identity_map(&Frame::from_addr(PhysAddr::new(addr as u64)));
        }
    }
}

impl VirtualAddessor for MappedVirtualAddessor {
    fn map(&mut self, page: &Page, frame: &Frame) {
        let addr_usize = (page.addr().as_u64() >> 12) as usize;
        let p1 = self
            .pml4_mut()
            .sub_table_create((addr_usize >> 27) & 0x1FF, self.mapped_addr)
            .sub_table_create((addr_usize >> 18) & 0x1FF, self.mapped_addr)
            .sub_table_create((addr_usize >> 9) & 0x1FF, self.mapped_addr);

        let entry = &mut p1[(addr_usize >> 0) & 0x1FF];
        if entry.is_present() {
            crate::instructions::tlb::invalidate(page);
        }

        entry.set(&frame, PageAttributes::PRESENT | PageAttributes::WRITABLE);
        trace!("Mapped {:?} to {:?}: {:?}", page, entry.frame(), entry);
    }

    fn unmap(&mut self, page: &Page) {
        //let addr_usize = (page.addr().as_u64() >> 12) as usize;
        // let entry = &mut self
        //     .pml4_mut()
        //     .sub_table_create((addr_usize >> 27) & 0x1FF)
        //     .sub_table_create((addr_usize >> 18) & 0x1FF)
        //     .sub_table_create((addr_usize >> 9) & 0x1FF)[(addr_usize >> 0) & 0x1FF];
        // entry.set_nonpresent();
        //trace!("Unmapped {:?} from {:?}: {:?}", page, entry.frame(), entry);
    }

    fn identity_map(&mut self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
    }

    fn swap_into(&self) {
        let frame = &self.pml4_frame();
        info!(
            "Writing page table manager's PML4 to CR3 register: {:?}.",
            frame
        );

        unsafe { crate::registers::CR3::write(frame, None) };
    }
}
