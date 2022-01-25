use crate::{addr_ty::Virtual, memory::UEFIMemoryDescriptor, Address};
use core::sync::atomic::{AtomicU32, Ordering};
use spin::RwLock;

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

    /// 'Borrows' a frame, incrementing the reference counter.
    fn borrow(&self) {
        // REMARK: Possibly handle case where 2^28 references have been made to a single frame?
        //         That does seem particularly unlikely, however.
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    /// 'Drops' a frame, decrementing the reference counter.
    fn drop(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    /// Locks a frame, setting the `locked` bit.
    fn lock(&self) {
        self.0.fetch_or(1 << Self::LOCKED_SHIFT, Ordering::AcqRel);
    }

    /// Frees a frame, unsetting the `locked` bit.
    fn free(&self) {
        self.0
            .fetch_and(!(1 << Self::LOCKED_SHIFT), Ordering::AcqRel);
    }

    /// Returns the frame data in a tuple.
    /// (frame type, reference count, is locked)
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

    /// Attempts to modify the frame type. There are various checks internally to
    /// ensure this is a valid operation.
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
pub enum FrameError {
    FrameUnusable(usize),
    FrameBorrowed(usize),
    FrameLocked(usize),
    FrameNotBorrowed(usize),
    FrameNotLocked(usize),
    NoFreeFrames,
}

pub struct FrameManager<'arr> {
    map: RwLock<&'arr mut [Frame]>,
    map_len: usize,
    total_memory: usize,
}

impl<'arr> FrameManager<'arr> {
    fn new(memory_map: &[UEFIMemoryDescriptor]) -> Self {
        // Calculates total (usable) system memory.
        let total_usable_memory = memory_map
            .iter()
            .filter(|descriptor| descriptor.ty != crate::memory::uefi::UEFIMemoryType::UNUSABLE)
            .map(|descriptor| descriptor.page_count * 0x1000)
            .sum::<u64>() as usize;
        // Calculates total system memory.
        let total_system_memory = memory_map
            .iter()
            .max_by_key(|descriptor| descriptor.phys_start)
            .map(|descriptor| {
                descriptor.phys_start.as_usize() + ((descriptor.page_count * 0x1000) as usize)
            })
            .unwrap();
        // Memory required to represent all system frames.
        let total_system_frames = crate::align_up_div(total_system_memory, 0x1000);
        let req_falloc_memory = total_system_frames * core::mem::size_of::<Frame>();
        let req_falloc_memory_frames = crate::align_up_div(req_falloc_memory, 0x1000);

        info!(
            "Frame allocator will manage {} MiB of system memory.",
            crate::memory::to_mibibytes(total_usable_memory)
        );

        // Find the best-fit descriptor for the falloc memory frames.
        let descriptor = memory_map
            .iter()
            .filter(|descriptor| !descriptor.should_reserve())
            .filter(|descriptor| descriptor.page_count >= (req_falloc_memory_frames as u64))
            .min_by_key(|descriptor| descriptor.page_count)
            .expect("Failed to find viable memory descriptor for frame allocator");
        let descriptor_ptr = descriptor.phys_start.as_usize() as *mut _;
        // Clear the memory of the chosen descriptor.
        unsafe { core::ptr::write_bytes(descriptor_ptr, 0, total_system_frames) };

        let falloc = Self {
            map: RwLock::new(unsafe {
                core::slice::from_raw_parts_mut(descriptor_ptr, total_system_frames)
            }),
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
            falloc
                .try_modify_type(frame_index, FrameType::Reserved)
                .unwrap();
            falloc.lock(frame_index).unwrap();
        }

        debug!("Reserving requsite frames from BIOS memory map.");
        let mut last_frame_end = 0;
        for descriptor in memory_map
            .iter()
            .filter(|d| d.phys_start.is_frame_aligned())
        {
            let frame_index = descriptor.phys_start.frame_index();
            let frame_count = descriptor.page_count as usize;

            // Checks for 'holes' in system memory which we shouldn't try to allocate to.
            for frame_index in last_frame_end..frame_index {
                falloc
                    .try_modify_type(frame_index, FrameType::Unusable)
                    .unwrap();
            }

            if descriptor.should_reserve() {
                falloc.lock_many(frame_index, frame_count).unwrap();

                for frame_index in frame_index..(frame_index + frame_count) {
                    falloc
                        .try_modify_type(frame_index, FrameType::Reserved)
                        .unwrap()
                }
            }

            last_frame_end = frame_index + frame_count;
        }

        falloc
    }

    pub fn lock(&self, index: usize) -> Result<usize, FrameError> {
        let frame = &mut self.map.write()[index];
        let (ty, ref_count, locked) = frame.data();

        if ty == FrameType::Unusable {
            Err(FrameError::FrameUnusable(index))
        } else if ref_count > 0 {
            Err(FrameError::FrameBorrowed(index))
        } else if locked {
            Err(FrameError::FrameLocked(index))
        } else {
            frame.lock();
            Ok(index)
        }
    }

    pub fn lock_many(&self, index: usize, count: usize) -> Result<usize, FrameError> {
        self.map
            .write()
            .iter()
            .skip(index)
            .take(count)
            .find_map(|frame| {
                let (ty, ref_count, locked) = frame.data();

                if ty == FrameType::Unusable {
                    Some(FrameError::FrameUnusable(index))
                } else if ref_count > 0 {
                    Some(FrameError::FrameBorrowed(index))
                } else if locked {
                    Some(FrameError::FrameLocked(index))
                } else {
                    frame.lock();
                    None
                }
            })
            .map_or(Ok(index), |error| Err(error))
    }

    pub fn free(&self, index: usize) -> Result<(), FrameError> {
        let frame = &self.map.read()[index];
        let (_, _, locked) = frame.data();

        if !locked {
            Err(FrameError::FrameNotLocked(index))
        } else {
            frame.free();
            Ok(())
        }
    }

    pub fn borrow(&self, index: usize) -> Result<usize, FrameError> {
        let frame = &self.map.read()[index];
        let (ty, _, locked) = frame.data();

        if ty == FrameType::Unusable {
            Err(FrameError::FrameUnusable(index))
        } else if locked {
            Err(FrameError::FrameLocked(index))
        } else {
            frame.borrow();
            Ok(index)
        }
    }

    pub fn drop(&self, index: usize) -> Result<(), FrameError> {
        let frame = &self.map.read()[index];
        let (_, ref_count, _) = frame.data();

        if ref_count == 0 {
            Err(FrameError::FrameNotBorrowed(index))
        } else {
            frame.drop();
            Ok(())
        }
    }

    pub fn lock_next(&self) -> Result<usize, FrameError> {
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
            .ok_or(FrameError::NoFreeFrames)
    }

    pub fn lock_next_many(&self, count: usize) -> Result<usize, FrameError> {
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
            Err(FrameError::NoFreeFrames)
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

    pub fn total_frame_count(&self) -> usize {
        self.map_len
    }

    pub fn iter<'outer>(&'arr self) -> FrameIterator<'outer, 'arr> {
        FrameIterator {
            map: &self.map,
            map_len: self.map_len,
            cur_index: 0,
        }
    }
}

pub struct FrameIterator<'lock, 'arr> {
    map: &'lock RwLock<&'arr mut [Frame]>,
    map_len: usize,
    cur_index: usize,
}

impl Iterator for FrameIterator<'_, '_> {
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

lazy_static::lazy_static! {
    pub static ref FRAME_MANAGER: FrameManager<'static> = {
        FrameManager::new(crate::BOOT_INFO.get().expect("`BOOT_INFO` invalid, cannot construct FrameManager").memory_map())
    };
}

pub fn virtual_map_offset() -> Address<Virtual> {
    Address::<Virtual>::new(crate::VADDR_HW_MAX - FRAME_MANAGER.total_memory())
}
