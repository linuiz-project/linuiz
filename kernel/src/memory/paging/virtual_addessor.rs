use core::cell::RefCell;

use crate::memory::{
    allocators::{global_memory_mut, total_memory_iter},
    paging::{Level4, PageAttributes, PageTable},
    Frame, Page,
};
use spin::Mutex;
use x86_64::{PhysAddr, VirtAddr};

pub struct VirtualAddressor {
    mapped_addr: VirtAddr,
    pml4_frame: RefCell<Frame>,
    guard: Mutex<usize>,
}

impl VirtualAddressor {
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
            pml4_frame: RefCell::new(global_memory_mut(|allocator| {
                allocator
                    .lock_next()
                    .expect("failed to lock frame for PML4 of MappedVirtualAddessor")
            })),
            guard: Mutex::new(0),
        }
    }

    fn pml4_mut(&self) -> &mut PageTable<Level4> {
        unsafe {
            &mut *self
                .mapped_addr
                .clone()
                .as_mut_ptr::<PageTable<Level4>>()
                .offset(self.pml4_frame.borrow().index() as isize)
        }
    }

    pub fn map(&self, page: &Page, frame: &Frame) {
        trace!("Mapping: {:?} to {:?}", page, frame);
        self.guard.lock();

        let offset = self.mapped_addr.clone();
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

    pub fn unmap(&self, page: &Page) {
        self.guard.lock();

        let offset = self.mapped_addr.clone();
        let addr = (page.addr().as_u64() >> 12) as usize;
        let entry = &mut self
            .pml4_mut()
            .sub_table_create((addr >> 27) & 0x1FF, offset)
            .sub_table_create((addr >> 18) & 0x1FF, offset)
            .sub_table_create((addr >> 9) & 0x1FF, offset)[(addr >> 0) & 0x1FF];

        entry.set_nonpresent();
        trace!("Unmapped {:?} from {:?}: {:?}", page, entry.frame(), entry);
    }

    pub fn identity_map(&self, frame: &Frame) {
        self.map(
            &Page::from_addr(VirtAddr::new(frame.addr().as_u64())),
            frame,
        );
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

        for addr in total_memory_iter().step_by(0x1000).map(|addr| addr as u64) {
            let virt_addr = VirtAddr::new(new_mapped_addr.as_u64() + addr);
            let phys_addr = PhysAddr::new(addr);
            self.map(&Page::from_addr(virt_addr), &Frame::from_addr(phys_addr));
        }

        self.mapped_addr = new_mapped_addr;
    }

    pub fn swap_into(&self) {
        let pml4_frame = &self.pml4_frame.borrow();
        info!(
            "Writing virtual addessor's PML4 to CR3 register: {:?}.",
            pml4_frame
        );

        unsafe { crate::registers::CR3::write(pml4_frame, None) };
    }
}

pub struct VirtualAddressorCell {
    value: Option<VirtualAddressor>,
}

impl VirtualAddressorCell {
    pub const fn empty() -> Self {
        Self { value: None }
    }

    pub fn new(virtual_addressor: VirtualAddressor) -> Self {
        Self {
            value: Some(virtual_addressor),
        }
    }

    pub fn replace(&mut self, virtual_addressor: VirtualAddressor) {
        match self.value {
            Some(_) => panic!("virtual addressor has already been confgiured"),
            None => self.value = Some(virtual_addressor),
        }
    }

    pub fn get(&self) -> &VirtualAddressor {
        self.value
            .as_ref()
            .expect("virtual addressor has not been configured")
    }

    pub fn get_mut(&mut self) -> &mut VirtualAddressor {
        self.value
            .as_mut()
            .expect("virtual addressor has not been configured")
    }
}

unsafe impl Sync for VirtualAddressorCell {}
