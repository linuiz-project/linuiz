use core::sync::atomic::{AtomicU8, Ordering};
use libkernel::Address;
use num_enum::TryFromPrimitive;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum FrameType {
    Usable = 0,
    Unusable,
    Reserved,
    MMIO,
    Kernel,
    FrameMap,
    BootReclaim,
    AcpiReclaim,
}

#[derive(Debug)]
#[repr(transparent)]
struct Frame(AtomicU8);

impl Frame {
    const PEEKED_BIT: u8 = 1 << 0;
    const LOCKED_BIT: u8 = 1 << 1;
    const FRAME_TYPE_SHIFT: u32 = 2;

    /// Locks a frame, setting the `locked` bit.
    #[inline]
    fn lock(&self) {
        self.0.fetch_or(Self::LOCKED_BIT, Ordering::Relaxed);
    }

    /// Frees a frame, unsetting the `locked` bit.
    #[inline]
    fn free(&self) {
        self.0.fetch_and(!Self::LOCKED_BIT, Ordering::Relaxed);
    }

    #[inline]
    fn try_peek(&self) -> bool {
        (self.0.fetch_or(Self::PEEKED_BIT, Ordering::Relaxed) & Self::PEEKED_BIT) == 0
    }

    #[inline]
    fn peek(&self) {
        while !self.try_peek() {
            core::hint::spin_loop();
        }
    }

    #[inline]
    fn unpeek(&self) {
        self.0.fetch_and(!Self::PEEKED_BIT, Ordering::Relaxed);
    }

    /// Returns the frame data in a tuple.
    /// (frame type, reference count, is locked)
    fn data(&self) -> (bool, FrameType) {
        let raw = self.0.load(Ordering::Relaxed);

        (
            (raw & Self::LOCKED_BIT) > 0,
            match FrameType::try_from(raw >> Self::FRAME_TYPE_SHIFT) {
                Ok(val) => val,
                Err(_) => panic!("{:#b}", raw),
            },
        )
    }

    /// Attempts to modify the frame type. There are various checks internally to
    /// ensure this is a valid operation.
    fn try_modify_type(&self, new_type: FrameType) -> Result<(), FrameError> {
        let raw = self.0.load(Ordering::Relaxed);
        let frame_type = FrameType::try_from(raw >> Self::FRAME_TYPE_SHIFT).unwrap();

        if frame_type == new_type {
            Ok(())
        } else if frame_type == FrameType::Usable
            || (new_type == FrameType::MMIO && matches!(frame_type, FrameType::Unusable | FrameType::Reserved))
            || (new_type == FrameType::Usable && matches!(frame_type, FrameType::BootReclaim | FrameType::AcpiReclaim))
        {
            self.0.store(((new_type as u8) << Self::FRAME_TYPE_SHIFT) | (raw & 0b11), Ordering::Release);
            if let Err(_) = FrameType::try_from(raw >> Self::FRAME_TYPE_SHIFT) {
                info!("{:b} {:?} -> {:?}", raw, frame_type, new_type);
            }
            Ok(())
        } else {
            Err(FrameError::TypeConversion { from: frame_type, to: new_type })
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameError {
    FrameUnusable,
    FrameLocked,
    FrameNotLocked,
    OutOfRange,
    TypeConversion { from: FrameType, to: FrameType },
    NoFreeFrames,
}

pub struct FrameManager<'arr>(&'arr [Frame]);

unsafe impl Sync for FrameManager<'_> {}

impl<'arr> FrameManager<'arr> {
    pub fn from_memory_map(
        memory_map: &[limine::NonNullPtr<limine::LimineMemmapEntry>],
        hhdm_addr: libkernel::Address<libkernel::Virtual>,
    ) -> Self {
        use limine::LimineMemoryMapEntryType;

        // Calculates total system memory.
        let total_system_memory = memory_map.last().map(|entry| entry.base + entry.len).unwrap();
        debug!("Total system memory: {:#X} bytes", total_system_memory);
        // Memory required to represent all system frames.
        let total_system_frames = libkernel::align_up_div(total_system_memory as usize, 0x1000);
        let req_falloc_memory = total_system_frames * core::mem::size_of::<Frame>();
        let req_falloc_memory_frames = libkernel::align_up_div(req_falloc_memory as usize, 0x1000);
        let req_falloc_memory_aligned = req_falloc_memory_frames * 0x1000;

        debug!("Required frame manager table memory: {:#X}", req_falloc_memory_aligned);

        // Find the best-fit descriptor for the falloc memory frames.
        let table_entry = memory_map
            .iter()
            .filter(|entry| {
                matches!(entry.typ, LimineMemoryMapEntryType::BootloaderReclaimable | LimineMemoryMapEntryType::Usable)
            })
            .find(|entry| entry.len >= (req_falloc_memory_aligned as u64))
            .expect("Failed to find viable memory descriptor for frame allocator");

        // Clear the memory of the chosen descriptor.
        unsafe { core::ptr::write_bytes(table_entry.base as *mut u8, 0, table_entry.len as usize) };

        let frame_table = unsafe {
            core::slice::from_raw_parts_mut((hhdm_addr.as_u64() + table_entry.base) as *mut Frame, total_system_frames)
        };

        let frame_ledger_range =
            (table_entry.base / 0x1000)..((table_entry.base / 0x1000) + (req_falloc_memory_frames as u64));
        for frame_index in frame_ledger_range {
            let frame = &mut frame_table[frame_index as usize];
            frame.try_modify_type(FrameType::FrameMap).unwrap();
            frame.lock();
        }

        debug!("Reserving requsite system frames.");
        let mut last_frame_end = 0;
        for entry in memory_map {
            assert_eq!(entry.base & 0xFFF, 0, "Memory map entry is not page-aligned: {:?}", entry);

            let start_index = entry.base / 0x1000;
            let frame_count = entry.len / 0x1000;

            // Checks for 'holes' in system memory which we shouldn't try to allocate to.
            for frame_index in last_frame_end..start_index {
                let frame = &mut frame_table[frame_index as usize];
                frame.try_modify_type(FrameType::Unusable).unwrap();
            }

            // Translate UEFI memory type to kernel frame type.
            let frame_ty = match entry.typ {
                LimineMemoryMapEntryType::Usable => FrameType::Usable,
                LimineMemoryMapEntryType::BootloaderReclaimable => FrameType::BootReclaim,
                LimineMemoryMapEntryType::AcpiReclaimable => FrameType::AcpiReclaim,
                LimineMemoryMapEntryType::KernelAndModules => FrameType::Kernel,
                LimineMemoryMapEntryType::Reserved => FrameType::Reserved,
                LimineMemoryMapEntryType::AcpiNvs | LimineMemoryMapEntryType::Framebuffer => FrameType::MMIO,
                LimineMemoryMapEntryType::BadMemory => FrameType::Unusable,
            };

            if frame_ty != FrameType::Usable {
                for frame_index in start_index..(start_index + frame_count) {
                    let frame = &mut frame_table[frame_index as usize];
                    frame.try_modify_type(frame_ty).unwrap();
                    frame.lock();
                }
            }

            last_frame_end = start_index + frame_count;
        }

        debug!("Successfully configured frame manager.");

        Self(frame_table)
    }

    fn with_table<T>(&self, func: impl FnOnce(&[Frame]) -> T) -> T {
        crate::interrupts::without(|| func(self.0))
    }

    pub fn lock(&self, frame: Address<libkernel::Frame>) -> Result<(), FrameError> {
        self.with_table(|table| match table.get(frame.index()) {
            Some(frame) => {
                frame.peek();

                let (locked, ty) = frame.data();
                let result = if ty == FrameType::Unusable {
                    Err(FrameError::FrameUnusable)
                } else if locked {
                    Err(FrameError::FrameLocked)
                } else {
                    frame.lock();
                    Ok(())
                };

                frame.unpeek();
                result
            }

            None => Err(FrameError::OutOfRange),
        })
    }

    pub fn lock_many(&self, base: Address<libkernel::Frame>, count: usize) -> Result<(), FrameError> {
        self.with_table(|table| {
            let frames = &table[base.index()..(base.index() + count)];
            frames.iter().for_each(Frame::peek);

            let result = if frames.iter().map(Frame::data).all(|(locked, ty)| {
                info!("LOCK_MANY {} {:?}", locked, ty);
                !locked && ty == FrameType::Usable
            }) {
                frames.iter().for_each(Frame::lock);
                Ok(())
            } else {
                Err(FrameError::FrameLocked)
            };

            frames.iter().for_each(Frame::unpeek);
            result
        })
    }

    pub fn free(&self, frame: Address<libkernel::Frame>) -> Result<(), FrameError> {
        self.with_table(|table| match table.get(frame.index()) {
            Some(frame) => {
                frame.peek();

                let (locked, _) = frame.data();
                let result = if locked {
                    frame.free();
                    Ok(())
                } else {
                    Err(FrameError::FrameNotLocked)
                };

                frame.unpeek();
                result
            }

            None => Err(FrameError::OutOfRange),
        })
    }

    pub fn lock_next(&self) -> Option<Address<libkernel::Frame>> {
        self.with_table(|table| {
            table.iter().enumerate().find_map(|(index, frame)| {
                if frame.try_peek() {
                    let (locked, ty) = frame.data();

                    let result = if !locked && ty == FrameType::Usable {
                        frame.lock();
                        Some(Address::<libkernel::Frame>::new_truncate((index * 0x1000) as u64))
                    } else {
                        None
                    };

                    frame.unpeek();
                    result
                } else {
                    info!("nopeek");
                    None
                }
            })
        })
    }

    pub fn lock_next_many(&self, count: usize) -> Result<Address<libkernel::Frame>, FrameError> {
        self.with_table(|table| {
            let mut start_index = 0;

            while start_index < (table.len() - count) {
                let sub_table = &table[start_index..(start_index + count)];
                sub_table.iter().for_each(Frame::peek);

                let result = match sub_table
                    .iter()
                    .enumerate()
                    .map(|(index, frame)| (index, frame.data()))
                    .find(|(_, (locked, ty))| *locked || *ty != FrameType::Usable)
                {
                    Some((index, _)) => {
                        start_index += index + 1;
                        None
                    }
                    None => {
                        sub_table.iter().for_each(Frame::lock);
                        Some(Address::<libkernel::Frame>::new_truncate((start_index * 0x1000) as u64))
                    }
                };

                sub_table.iter().for_each(Frame::unpeek);
                if let Some(address) = result {
                    return Ok(address);
                }
            }

            Err(FrameError::NoFreeFrames)
        })
    }

    pub fn try_modify_type(&self, frame: Address<libkernel::Frame>, new_type: FrameType) -> Result<(), FrameError> {
        self.with_table(|table| {
            table.get(frame.index()).ok_or(FrameError::OutOfRange).and_then(|frame| frame.try_modify_type(new_type))
        })
    }

    pub fn get_frame_info(&self, frame: Address<libkernel::Frame>) -> Option<(bool, FrameType)> {
        self.with_table(|table| table.get(frame.index()).map(Frame::data))
    }

    pub fn iter(&'arr self) -> FrameIterator<'arr> {
        FrameIterator { table: self.0, cur_index: 0 }
    }
}

pub struct FrameIterator<'arr> {
    table: &'arr [Frame],
    cur_index: usize,
}

impl Iterator for FrameIterator<'_> {
    type Item = (bool, FrameType);

    fn next(&mut self) -> Option<Self::Item> {
        crate::interrupts::without(|| {
            if self.cur_index < self.table.len() {
                let cur_index = self.cur_index;
                self.cur_index += 1;

                Some(self.table[cur_index].data())
            } else {
                None
            }
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.table.len()))
    }
}

impl ExactSizeIterator for FrameIterator<'_> {}
