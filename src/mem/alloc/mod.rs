use crate::mem::{
    HigherHalfDirectMap,
    pmm::{NextFrameError, PhysicalMemoryManager},
};
use core::{num::NonZero, ptr::NonNull};
use libsys::constants::page_size;

mod global;



// pub static KERNEL_ALLOCATOR: SlabAllocator<FrameAllocator> =
// SlabAllocator::new_in(FrameAllocator);

pub fn allocate_kernel_stack(pages: NonZero<usize>) -> Result<NonNull<u8>, NextFrameError> {
    let base_address = PhysicalMemoryManager::next_free(pages, false)?;
    let memory_size = pages.get() * page_size();

    let ptr_offset = HigherHalfDirectMap::offset(base_address.get().get());
    let ptr = NonNull::<u8>::with_exposed_provenance(ptr_offset);
    let slice_ptr = NonNull::slice_from_raw_parts(ptr, memory_size);

    // Safety: `self.0` cannot have a higher index than `self.0.len() - 1`.
    let top_ptr = unsafe { slice_ptr.get_unchecked_mut(slice_ptr.len() - 1) };

    Ok(top_ptr)
}
