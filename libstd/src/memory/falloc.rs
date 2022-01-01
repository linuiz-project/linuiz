use crate::{addr_ty::Virtual, cell::SyncOnceCell, memory::UEFIMemoryDescriptor, Address};
use core::sync::atomic::{AtomicU32, Ordering};
use spin::RwLock;

static DEFAULT_FALLOCATOR: SyncOnceCell<FrameAllocator> = SyncOnceCell::new();

pub unsafe fn load_new(memory_map: &[UEFIMemoryDescriptor]) {
    DEFAULT_FALLOCATOR
        .set(FrameAllocator::new(memory_map))
        .unwrap_or_else(|_| {
            panic!("Frame allocator can only be loaded once.");
        })
}

pub fn get() -> &'static FrameAllocator<'static> {
    DEFAULT_FALLOCATOR
        .get()
        .expect("frame allocator has not been configured")
}

pub fn virtual_map_offset() -> Address<Virtual> {
    Address::<Virtual>::new(crate::VADDR_HW_MAX - get().total_memory())
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Usable = 0,
    Unusable,
    Reserved,
}

#[derive(Debug)]
#[repr(transparent)]
struct Frame(AtomicU32);

impl Frame {
    const FRAME_TYPE_SHIFT: u32 = 30;
    const LOCKED_SHIFT: u32 = 28;
    const REF_COUNT_MASK: u32 = 0xFFFFFFF; // first 28 bits

    fn borrow(&self) {
        // REMARK: Possibly handle case where 2^28 references have been made to a single frame?
        //          That does seem particularly unlikely, however.
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    fn drop(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    fn lock(&self) {
        self.0.fetch_or(1 << Self::LOCKED_SHIFT, Ordering::AcqRel);
    }

    fn free(&self) {
        self.0
            .fetch_and(!(1 << Self::LOCKED_SHIFT), Ordering::AcqRel);
    }

    fn data(&self) -> (FrameType, u32, bool) {
        let raw = self.0.load(Ordering::Relaxed);

        (
            match raw >> Self::FRAME_TYPE_SHIFT {
                0 => FrameType::Usable,
                1 => FrameType::Unusable,
                2 => FrameType::Reserved,
                _ => panic!(""),
            },
            raw & Self::REF_COUNT_MASK,
            ((raw >> Self::LOCKED_SHIFT) & 1) > 0,
        )
    }

    fn modify_type(&self, new_type: FrameType) -> Result<(), ()> {
        let raw = self.0.load(Ordering::Relaxed);

        if (raw >> Self::FRAME_TYPE_SHIFT) == 0 {
            // Usable / Generic
            self.0.store(
                ((new_type as u32) << Self::FRAME_TYPE_SHIFT)
                    | (raw & ((1 << Self::FRAME_TYPE_SHIFT) - 1)),
                Ordering::Release,
            );

            Ok(())
        } else {
            Err(())
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FallocError {
    FrameUnusable(usize),
    FrameBorrowed(usize),
    FrameLocked(usize),
    FrameNotBorrowed(usize),
    FrameNotLocked(usize),
    NoFreeFrames,
}

/// Structure for deterministically reserving, locking, and freeing RAM frames.
///
/// Deterministic Frame Lifetimes
/// -----------------------------
/// Deterministic frame lifetimes is the concept that a frame's lifetime should be
/// carefully controlled to ensure its behaviour matches how its used in hardware.
///
/// As an example, a page table entry locks a frame to create a sub-table. Conceptually,
/// the sub-table's entry in the page table owns the frame that sub-table exists on. It
/// follows that that entry should control the lifetime of that frame.
///
/// This example encapsulates the core idea, that the `Frame` struct shouldn't be instantiated
/// out of thin air. Its creation should be carefully controlled, to ensure each individual frame's
/// lifetime matches up with how it is used or consumed in hardware and software.
pub struct FrameAllocator<'arr> {
    map: RwLock<&'arr mut [Frame]>,
    map_len: usize,
    total_memory: usize,
}

impl<'arr> FrameAllocator<'arr> {
    fn new(memory_map: &[UEFIMemoryDescriptor]) -> Self {
        // Calculates total (usable) system memory.
        let total_usable_memory = memory_map
            .iter()
            .filter(|descriptor| !descriptor.should_reserve())
            .map(|descriptor| descriptor.page_count * 0x1000)
            .sum::<u64>() as usize;
        // Calculates total system memory.
        let total_system_memory = memory_map
            .iter()
            .filter(|descriptor| !descriptor.should_reserve())
            .max_by_key(|descriptor| descriptor.phys_start)
            .map(|descriptor| {
                descriptor.phys_start.as_usize() + ((descriptor.page_count * 0x1000) as usize)
            })
            .expect("no descriptor with max value");
        // Memory required to represent all system frames.
        let total_system_frames = total_system_memory / 0x1000;
        let req_falloc_memory = total_system_frames * core::mem::size_of::<Frame>();
        let req_falloc_memory_frames = crate::align_up_div(req_falloc_memory, 0x1000);

        info!(
            "Frame allocator will manage {} MB of system memory.",
            crate::memory::to_mibibytes(total_usable_memory)
        );

        let descriptor = memory_map
            .iter()
            .filter(|descriptor| !descriptor.should_reserve())
            .find(|descriptor| descriptor.page_count >= (req_falloc_memory_frames as u64))
            .expect("Failed to find viable memory descriptor for frame allocator");

        unsafe {
            let descriptor_ptr = descriptor.phys_start.as_usize() as *mut _;
            core::ptr::write_bytes(descriptor_ptr, 0, total_system_frames);

            let falloc = Self {
                map: RwLock::new(core::slice::from_raw_parts_mut(
                    descriptor_ptr,
                    total_system_frames,
                )),
                map_len: total_system_frames,
                total_memory: total_system_memory,
            };

            falloc.try_modify_type(0, FrameType::Unusable).unwrap();
            let frame_range = descriptor.phys_start.frame_index()
                ..(descriptor.phys_start.frame_index() + req_falloc_memory_frames);
            debug!(
                "Locking frames {:?} to facilitate static frame allocator map.",
                frame_range
            );
            for frame_index in frame_range {
                falloc.lock(frame_index).unwrap();
            }

            falloc
        }
    }

    pub fn lock(&self, index: usize) -> Result<usize, FallocError> {
        let frame = &mut self.map.write()[index];
        let (ty, ref_count, locked) = frame.data();

        if ty == FrameType::Unusable {
            Err(FallocError::FrameUnusable(index))
        } else if ref_count > 0 {
            Err(FallocError::FrameBorrowed(index))
        } else if locked {
            Err(FallocError::FrameLocked(index))
        } else {
            frame.lock();
            Ok(index)
        }
    }

    pub fn lock_many(&self, index: usize, count: usize) -> Result<usize, FallocError> {
        self.map
            .write()
            .iter()
            .skip(index)
            .take(count)
            .find_map(|frame| {
                let (ty, ref_count, locked) = frame.data();

                if ty == FrameType::Unusable {
                    Some(FallocError::FrameUnusable(index))
                } else if ref_count > 0 {
                    Some(FallocError::FrameBorrowed(index))
                } else if locked {
                    Some(FallocError::FrameLocked(index))
                } else {
                    frame.lock();
                    None
                }
            })
            .map_or(Ok(index), |error| Err(error))
    }

    pub fn free(&self, index: usize) -> Result<(), FallocError> {
        let frame = &self.map.read()[index];
        let (_, _, locked) = frame.data();

        if !locked {
            Err(FallocError::FrameNotLocked(index))
        } else {
            frame.free();
            Ok(())
        }
    }

    pub fn borrow(&self, index: usize) -> Result<usize, FallocError> {
        let frame = &self.map.read()[index];
        let (ty, _, locked) = frame.data();

        if ty == FrameType::Unusable {
            Err(FallocError::FrameUnusable(index))
        } else if locked {
            Err(FallocError::FrameLocked(index))
        } else {
            frame.borrow();
            Ok(index)
        }
    }

    pub fn drop(&self, index: usize) -> Result<(), FallocError> {
        let frame = &self.map.read()[index];
        let (_, ref_count, _) = frame.data();

        if ref_count == 0 {
            Err(FallocError::FrameNotBorrowed(index))
        } else {
            frame.drop();
            Ok(())
        }
    }

    pub fn lock_next(&self) -> Result<usize, FallocError> {
        self.map
            .read()
            .iter()
            .enumerate()
            .find_map(|(index, frame)| {
                let (ty, ref_count, locked) = frame.data();

                if ty == FrameType::Usable && ref_count == 0 && !locked {
                    frame.lock();
                    Some(index)
                } else {
                    None
                }
            })
            .ok_or(FallocError::NoFreeFrames)
    }

    pub fn lock_next_many(&self, count: usize) -> Result<usize, FallocError> {
        let map = self.map.write();

        let mut start_index = 0;
        let mut current_run = 0;
        for (index, frame) in map.iter().enumerate() {
            let (ty, ref_count, locked) = frame.data();

            if ty == FrameType::Usable && ref_count == 0 && !locked {
                current_run += 1;

                if current_run == count {
                    break;
                }
            } else {
                current_run = 0;
                start_index = index + 1;
            }
        }

        if current_run < count {
            Err(FallocError::NoFreeFrames)
        } else {
            map.iter()
                .skip(start_index)
                .take(count)
                .for_each(|frame| frame.lock());

            Ok(start_index)
        }
    }

    pub fn try_modify_type(
        &self,
        index: usize,
        new_type: FrameType,
    ) -> core::result::Result<(), ()> {
        self.map.read()[index].modify_type(new_type)
    }

    /// Total memory of a given type represented by frame allocator. If `None` is
    ///  provided for type, the total of all memory types is returned instead.
    pub fn total_memory(&self) -> usize {
        self.total_memory
    }

    pub fn iter<'outer>(&'arr self) -> FallocIterator<'outer, 'arr> {
        FallocIterator {
            map: &self.map,
            map_len: self.map_len,
            cur_index: 0,
        }
    }

    #[cfg(debug_assertions)]
    pub fn debug_log_elements(&self) {
        self.map.debug_log_elements();
    }
}

pub struct FallocIterator<'lock, 'arr> {
    map: &'lock RwLock<&'arr mut [Frame]>,
    map_len: usize,
    cur_index: usize,
}

impl Iterator for FallocIterator<'_, '_> {
    type Item = (FrameType, u32, bool);

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_index < self.map_len {
            let cur_index = self.cur_index;
            self.cur_index += 1;

            Some(self.map.read()[cur_index].data())
        } else {
            None
        }
    }
}
