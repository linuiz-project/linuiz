use crate::{memory::uefi, Address, Virtual};
use core::sync::atomic::{AtomicU32, Ordering};
use num_enum::TryFromPrimitive;
use spin::RwLock;

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameType {
    Usable = 0,
    Unusable,
    Reserved,
    MMIO,
    // TODO possibly ACPI reclaim?
    Kernel,
    FrameMap,
}

#[derive(Debug)]
#[repr(transparent)]
struct Frame(AtomicU32);

impl Frame {
    const REF_COUNT_MASK: u32 = 0xFFFF;
    const PEEKED_BIT: u32 = 1 << 16;
    const LOCKED_BIT: u32 = 1 << 17;
    const FRAME_TYPE_SHIFT: u32 = 26;

    #[inline]
    const fn empty() -> Self {
        Self(AtomicU32::new(0))
    }

    /// 'Borrows' a frame, incrementing the reference counter.
    #[inline]
    fn borrow(&self) {
        // REMARK: Possibly handle case where 2^16 references have been made to a single frame?
        //         That does seem particularly unlikely, however.
        self.0.fetch_add(1, Ordering::AcqRel);
    }

    /// 'Drops' a frame, decrementing the reference counter.
    #[inline]
    fn drop(&self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }

    /// Locks a frame, setting the `locked` bit.
    #[inline]
    fn lock(&self) {
        self.0.fetch_or(Self::LOCKED_BIT, Ordering::AcqRel);
    }

    /// Frees a frame, unsetting the `locked` bit.
    #[inline]
    fn free(&self) {
        self.0.fetch_and(!Self::LOCKED_BIT, Ordering::AcqRel);
    }

    #[inline]
    fn try_peek(&self) -> bool {
        (self.0.fetch_or(Self::PEEKED_BIT, Ordering::AcqRel) & Self::PEEKED_BIT) == 0
    }

    #[inline]
    fn peek(&self) {
        while !self.try_peek() {}
    }

    #[inline]
    fn unpeek(&self) {
        self.0.fetch_and(!Self::PEEKED_BIT, Ordering::AcqRel);
    }

    /// Returns the frame data in a tuple.
    /// (frame type, reference count, is locked)
    fn data(&self) -> (FrameType, u32, bool) {
        let raw = self.0.load(Ordering::Relaxed);

        (
            FrameType::try_from(raw >> Self::FRAME_TYPE_SHIFT).unwrap(),
            raw & Self::REF_COUNT_MASK,
            (raw & Self::LOCKED_BIT) > 0,
        )
    }

    /// Attempts to modify the frame type. There are various checks internally to
    /// ensure this is a valid operation.
    fn modify_type(&self, new_type: FrameType) -> Result<(), FrameError> {
        let raw = self.0.load(Ordering::Relaxed);
        let frame_type = FrameType::try_from(raw >> Self::FRAME_TYPE_SHIFT).unwrap();

        if frame_type == new_type {
            Ok(())
        }
        // If frame is already usable ...
        else if frame_type == FrameType::Usable
        // or if frame is not, but new type is MMIO ...
            || (new_type == FrameType::MMIO
                && matches!(frame_type, FrameType::Unusable | FrameType::Reserved))
        {
            // Change frame type.

            // Usable / Generic
            self.0.store(
                ((new_type as u32) << Self::FRAME_TYPE_SHIFT)
                    | (raw & ((1 << Self::FRAME_TYPE_SHIFT) - 1)),
                Ordering::Release,
            );

            Ok(())
        } else {
            Err(FrameError::TypeConversion {
                from: frame_type,
                to: new_type,
            })
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
    OutOfRange(usize),
    TypeConversion { from: FrameType, to: FrameType },
    NoFreeFrames,
}

pub struct FrameManager<'arr> {
    map: RwLock<&'arr mut [Frame]>,
}

unsafe impl Send for FrameManager<'_> {}
unsafe impl Sync for FrameManager<'_> {}

impl<'arr> FrameManager<'arr> {
    pub fn from_mmap(memory_map: &[uefi::MemoryDescriptor]) -> Self {
        // Calculates total (usable) system memory.
        let total_usable_memory = memory_map
            .iter()
            .filter(|descriptor| descriptor.ty != crate::memory::uefi::MemoryType::UNUSABLE)
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
            crate::to_mibibytes(total_usable_memory)
        );

        // Find the best-fit descriptor for the falloc memory frames.
        let descriptor = memory_map
            .iter()
            .filter(|descriptor| {
                matches!(
                    descriptor.ty,
                    uefi::MemoryType::CONVENTIONAL
                        | uefi::MemoryType::BOOT_SERVICES_CODE
                        | uefi::MemoryType::BOOT_SERVICES_DATA
                        | uefi::MemoryType::LOADER_CODE
                        | uefi::MemoryType::LOADER_DATA
                )
            })
            .filter(|descriptor| descriptor.page_count >= (req_falloc_memory_frames as u64))
            .min_by_key(|descriptor| descriptor.page_count)
            .expect("Failed to find viable memory descriptor for frame allocator");
        // Clear the memory of the chosen descriptor.
        unsafe {
            core::ptr::write_bytes(
                descriptor.phys_start.as_usize() as *mut u8,
                0,
                req_falloc_memory,
            )
        };

        let falloc = Self {
            map: RwLock::new(unsafe {
                core::slice::from_raw_parts_mut(
                    descriptor.phys_start.as_usize() as *mut _,
                    total_system_frames,
                )
            }),
        };

        let frame_ledger_range = descriptor.phys_start.frame_index()
            ..(descriptor.phys_start.frame_index() + req_falloc_memory_frames);
        debug!(
            "Locking frames {:?} to facilitate static frame allocator map.",
            frame_ledger_range
        );
        for frame_index in frame_ledger_range {
            falloc
                .try_modify_type(frame_index, FrameType::FrameMap)
                .unwrap();
            falloc.lock(frame_index).unwrap();
        }

        // Modify the null frame to never be used.
        falloc.try_modify_type(0, FrameType::Reserved).unwrap();

        debug!("Reserving requsite system frames.");
        let mut last_frame_end = 0;
        for descriptor in memory_map
            .iter()
            .filter(|d| d.phys_start.is_frame_aligned())
        {
            let start_index = descriptor.phys_start.frame_index();
            let frame_count = descriptor.page_count as usize;

            // Checks for 'holes' in system memory which we shouldn't try to allocate to.
            for frame_index in last_frame_end..start_index {
                falloc
                    .try_modify_type(frame_index, FrameType::Unusable)
                    .unwrap();
            }

            if descriptor.should_reserve() {
                // Translate UEFI memory type to kernel frame type.
                let frame_ty = match descriptor.ty {
                    uefi::MemoryType::UNUSABLE => FrameType::Unusable,
                    uefi::MemoryType::MMIO_PORT_SPACE | uefi::MemoryType::MMIO => FrameType::MMIO,
                    uefi::MemoryType::KERNEL => FrameType::Kernel,
                    _ => FrameType::Reserved,
                };

                for frame_index in start_index..(start_index + frame_count) {
                    falloc.try_modify_type(frame_index, frame_ty).unwrap();
                    falloc.lock(frame_index).unwrap();
                }
            }

            last_frame_end = start_index + frame_count;
        }

        falloc
    }

    pub fn phys_mem_offset(&self) -> Address<Virtual> {
        Address::<Virtual>::new(crate::VADDR_HW_MAX - self.total_memory())
    }

    pub fn lock(&self, index: usize) -> Result<usize, FrameError> {
        if let Some(frame) = self.map.read().get(index) {
            frame.peek();

            let (ty, ref_count, locked) = frame.data();

            let result = if ty == FrameType::Unusable {
                Err(FrameError::FrameUnusable(index))
            } else if ref_count > 0 {
                Err(FrameError::FrameBorrowed(index))
            } else if locked {
                Err(FrameError::FrameLocked(index))
            } else {
                frame.lock();
                Ok(index)
            };

            frame.unpeek();
            result
        } else {
            Err(FrameError::OutOfRange(index))
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
        if let Some(frame) = self.map.read().get(index) {
            frame.peek();

            let (_, _, locked) = frame.data();

            let result = if !locked {
                Err(FrameError::FrameNotLocked(index))
            } else {
                frame.free();
                Ok(())
            };

            frame.unpeek();
            result
        } else {
            Err(FrameError::OutOfRange(index))
        }
    }

    pub fn borrow(&self, index: usize) -> Result<usize, FrameError> {
        if let Some(frame) = self.map.read().get(index) {
            let (ty, _, locked) = frame.data();

            if ty == FrameType::Unusable {
                Err(FrameError::FrameUnusable(index))
            } else if locked {
                Err(FrameError::FrameLocked(index))
            } else {
                frame.borrow();
                Ok(index)
            }
        } else {
            Err(FrameError::OutOfRange(index))
        }
    }

    pub fn drop(&self, index: usize) -> Result<(), FrameError> {
        if let Some(frame) = self.map.read().get(index) {
            let (_, ref_count, _) = frame.data();

            if ref_count == 0 {
                Err(FrameError::FrameNotBorrowed(index))
            } else {
                frame.drop();
                Ok(())
            }
        } else {
            Err(FrameError::OutOfRange(index))
        }
    }

    pub fn lock_next(&self) -> Result<usize, FrameError> {
        self.map
            .read()
            .iter()
            .enumerate()
            .find_map(|(index, frame)| {
                if frame.try_peek() {
                    let (ty, ref_count, locked) = frame.data();

                    if ty == FrameType::Usable && ref_count == 0 && !locked {
                        frame.lock();
                        frame.unpeek();
                        Some(index)
                    } else {
                        frame.unpeek();
                        None
                    }
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
            map.iter().skip(start_index).take(count).for_each(|frame| {
                frame.lock();
            });

            Ok(start_index)
        }
    }

    pub fn try_modify_type(
        &self,
        index: usize,
        new_type: FrameType,
    ) -> core::result::Result<(), FrameError> {
        self.map
            .read()
            .get(index)
            .ok_or(FrameError::OutOfRange(index))
            .and_then(|frame| frame.modify_type(new_type))
    }

    /// Total memory of a given type represented by frame allocator. If `None` is
    ///  provided for type, the total of all memory types is returned instead.
    pub fn total_memory(&self) -> usize {
        self.map.read().len() * 0x1000
    }

    pub fn total_frames(&self) -> usize {
        self.map.read().len()
    }

    pub fn map_pages(&self) -> core::ops::Range<super::Page> {
        let map_read = self.map.read();
        let ptr = map_read.as_ptr() as usize;

        super::Page::range(
            ptr / 0x1000,
            (ptr + (map_read.len() * core::mem::size_of::<Frame>())) / 0x1000,
        )
    }

    pub fn iter(&'arr self) -> FrameIterator<'arr> {
        FrameIterator {
            map: &self.map,
            cur_index: 0,
        }
    }
}

pub struct FrameIterator<'arr> {
    map: &'arr RwLock<&'arr mut [Frame]>,
    cur_index: usize,
}

impl Iterator for FrameIterator<'_> {
    type Item = (FrameType, u32, bool);

    fn next(&mut self) -> Option<Self::Item> {
        let map_read = self.map.read();
        if self.cur_index < map_read.len() {
            let cur_index = self.cur_index;
            self.cur_index += 1;

            Some(map_read[cur_index].data())
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.map.read().len()))
    }
}

impl ExactSizeIterator for FrameIterator<'_> {}
