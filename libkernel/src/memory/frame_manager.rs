use core::sync::atomic::{AtomicU32, Ordering};
use num_enum::TryFromPrimitive;
use spin::RwLock;

#[derive(Debug, PartialEq, Eq)]
pub enum FrameOwnership {
    None,
    Borrowed,
    Locked
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameType {
    Usable = 0,
    Unusable,
    Reserved,
    MMIO,
    Kernel,
    FrameMap,
    BootReclaim,
    ACPIReclaim,
}

#[derive(Debug)]
#[repr(transparent)]
struct Frame(AtomicU32);

impl Frame {
    const REF_COUNT_MASK: u32 = 0xFFFF;
    const PEEKED_BIT: u32 = 1 << 16;
    const LOCKED_BIT: u32 = 1 << 17;
    const FRAME_TYPE_SHIFT: u32 = 26;

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
            || (new_type == FrameType::Usable 
                && matches!(frame_type, FrameType::BootReclaim | FrameType::ACPIReclaim))
        {
            // Change frame type.
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
    pub fn from_mmap(memory_map: &[limine::LimineMemmapEntry]) -> Self {
        use limine::LimineMemoryMapEntryType;

        // Calculates total system memory.
        let total_system_memory = memory_map
            .last()
            .map(|entry| entry.base + entry.len)
            .unwrap();
        trace!("Total system memory: {:#X} bytes", total_system_memory);
        // Memory required to represent all system frames.
        let total_system_frames = crate::align_up_div(total_system_memory as usize, 0x1000);
        let req_falloc_memory = total_system_frames * core::mem::size_of::<Frame>();
        let req_falloc_memory_frames = crate::align_up_div(req_falloc_memory as usize, 0x1000);
        let req_falloc_memory_aligned = req_falloc_memory_frames * 0x1000;

        trace!(
            "Required frame manager map memory: {:#X}",
            req_falloc_memory_aligned
        );

        // Find the best-fit descriptor for the falloc memory frames.
        let map_entry = memory_map
            .iter()
            .filter(|entry| {
                matches!(
                    entry.typ,
                    LimineMemoryMapEntryType::BootloaderReclaimable
                        | LimineMemoryMapEntryType::Usable
                )
            })
            .find(|entry| entry.len >= (req_falloc_memory_aligned as u64))
            .expect("Failed to find viable memory descriptor for frame allocator");

            trace!("Found entry for frame manager map: {:?}", map_entry);

        // Clear the memory of the chosen descriptor.
        unsafe { core::ptr::write_bytes(map_entry.base as *mut u8, 0, req_falloc_memory_aligned) };

        let falloc = Self {
            map: RwLock::new(unsafe {
                core::slice::from_raw_parts_mut(map_entry.base as *mut _, total_system_frames)
            }),
        };

        let frame_ledger_range = (map_entry.base / 0x1000)
            ..((map_entry.base / 0x1000) + (req_falloc_memory_frames as u64));
            trace!(
            "Locking frames {:?} to facilitate static frame allocator map.",
            frame_ledger_range
        );
        for frame_index in frame_ledger_range {
            falloc
                .try_modify_type(frame_index as usize, FrameType::FrameMap)
                .unwrap();
            falloc.lock(frame_index as usize).unwrap();
        }

        trace!("Reserving requsite system frames.");
        let mut last_frame_end = 0;
        for entry in memory_map {
            assert_eq!(
                entry.base & 0xFFF,
                0,
                "Memory map entry is not page-aligned: {:?}",
                entry
            );

            let start_index = entry.base / 0x1000;
            let frame_count = entry.len / 0x1000;

            // Checks for 'holes' in system memory which we shouldn't try to allocate to.
            for frame_index in last_frame_end..start_index {
                falloc
                    .try_modify_type(frame_index as usize, FrameType::Unusable)
                    .unwrap();
            }

            // Translate UEFI memory type to kernel frame type.
            let frame_ty = match entry.typ {
                LimineMemoryMapEntryType::Usable => FrameType::Usable,
                LimineMemoryMapEntryType::BootloaderReclaimable => FrameType::BootReclaim,
                LimineMemoryMapEntryType::AcpiReclaimable => FrameType::ACPIReclaim,
                LimineMemoryMapEntryType::KernelAndModules => FrameType::Kernel,
                LimineMemoryMapEntryType::Reserved => FrameType::Reserved,
                LimineMemoryMapEntryType::AcpiNvs | LimineMemoryMapEntryType::Framebuffer => {
                    FrameType::MMIO
                }
                LimineMemoryMapEntryType::BadMemory => FrameType::Unusable,
            };

            if frame_ty != FrameType::Usable {
                for frame_index in start_index..(start_index + frame_count) {
                    falloc
                        .try_modify_type(frame_index as usize, frame_ty)
                        .unwrap();
                    falloc.lock(frame_index as usize).unwrap();
                }
            }

            last_frame_end = start_index + frame_count;
        }

        trace!("Successfully configured frame manager.");

        falloc
    }

    pub unsafe fn slide_table_base(&self, slide: usize) {
        let mut map_write = self.map.write();
        let new_map_base = map_write
            .as_mut_ptr()
            .cast::<u8>()
            .add(slide)
            .cast::<Frame>();
        *map_write = core::slice::from_raw_parts_mut(new_map_base, map_write.len());
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

    pub fn force_modify_type(&self, index: usize, new_type: FrameType) -> core::result::Result<(), FrameError> {
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
