use bit_field::BitField;
use core::{
    alloc::{AllocError, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
    sync::atomic::Ordering,
};
use libcommon::{Address, Frame};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    /// There are not enough free frames to satisfy the request.
    NoneFree,
    /// Given alignment is invalid (e.g. not a power-of-two).
    InvalidAlignment,
    /// The provided frame index was out of bounds of the frame table.
    OutOfBounds,
    /// Attempted to lock a frame that wasn't free.
    NotFree,
    /// Attempted to free a frame that wasn't locked.
    NotLocked,

    Unknown,
}

pub type Result<T> = core::result::Result<T, Error>;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Unusable,
    Generic,
    Reserved,
    BootReclaim,
    AcpiReclaim,
}

impl FrameType {
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Unusable,
            1 => Self::Generic,
            2 => Self::Reserved,
            3 => Self::BootReclaim,
            4 => Self::AcpiReclaim,
            _ => unimplemented!(),
        }
    }

    const fn as_u8(self) -> u8 {
        match self {
            FrameType::Unusable => 0,
            FrameType::Generic => 1,
            FrameType::Reserved => 2,
            FrameType::BootReclaim => 3,
            FrameType::AcpiReclaim => 4,
        }
    }
}

#[derive(Debug)]
pub struct FrameData(core::sync::atomic::AtomicU8);

impl FrameData {
    const LOCKED_SHIFT: usize = 7;
    const PEEKED_SHIFT: usize = 6;
    const LOCKED_BIT: u8 = 1 << Self::LOCKED_SHIFT;
    const PEEKED_BIT: u8 = 1 << Self::PEEKED_SHIFT;
    const TYPE_RANGE: core::ops::Range<usize> = 0..4;

    #[inline]
    fn lock(&self) {
        let lock_result = self.0.fetch_or(Self::LOCKED_BIT, Ordering::AcqRel);
        debug_assert!(!lock_result.get_bit(Self::LOCKED_SHIFT));
    }

    #[inline]
    fn free(&self) {
        let free_result = self.0.fetch_xor(Self::LOCKED_BIT, Ordering::AcqRel);
        debug_assert!(free_result.get_bit(Self::LOCKED_SHIFT));
    }

    #[inline]
    fn try_peek(&self) -> bool {
        !self.0.fetch_or(Self::PEEKED_BIT, Ordering::AcqRel).get_bit(Self::PEEKED_SHIFT)
    }

    #[inline]
    fn peek(&self) {
        while !self.try_peek() {
            core::hint::spin_loop();
        }
    }

    #[inline]
    fn unpeek(&self) {
        let unpeek_result = self.0.fetch_and(!Self::PEEKED_BIT, Ordering::AcqRel);
        debug_assert!(unpeek_result.get_bit(Self::PEEKED_SHIFT));
    }

    #[inline]
    fn set_type(&self, new_type: FrameType) {
        debug_assert!(self.0.load(Ordering::Acquire).get_bit(Self::PEEKED_SHIFT));

        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |mut value| {
                Some(*value.set_bits(Self::TYPE_RANGE, new_type.as_u8()))
            })
            .ok();
    }

    fn data(&self) -> (bool, FrameType) {
        debug_assert!(self.0.load(Ordering::Acquire).get_bit(Self::PEEKED_SHIFT));

        let raw = self.0.load(Ordering::Relaxed);
        (raw.get_bit(Self::LOCKED_SHIFT), FrameType::from_u8(raw.get_bits(Self::TYPE_RANGE)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemoryMapping {
    pub base: usize,
    pub len: usize,
    pub typ: FrameType,
}

pub struct PhysicalMemoryManager<'a> {
    table: &'a [FrameData],
    physical_memory: NonNull<u8>,
}

// ### Safety: Type uses entirely atomic operations.
unsafe impl Send for PhysicalMemoryManager<'_> {}
// ### Safety: Type uses entirely atomic operations.
unsafe impl Sync for PhysicalMemoryManager<'_> {}

impl PhysicalMemoryManager<'_> {
    // ### Safety: Caller must guarantee the physical mapped address is valid.
    pub unsafe fn from_memory_map(
        memory_map: impl ExactSizeIterator<Item = MemoryMapping>,
        physical_memory: NonNull<u8>,
    ) -> Option<Self> {
        let (memory_map, memory_map_len) = {
            let memory_map_len = memory_map.len();
            // # Remark
            // 64 possible memory map entries feels like a reasonable limit.
            // If this limit becomes constrining, it may be increased or set
            // dynamically (at compile-time) with a build option.
            // #### notation: lomem feature
            let mut array = [MemoryMapping { base: 0, len: 0, typ: FrameType::Unusable }; 64];
            memory_map.enumerate().for_each(|(index, entry)| array[index] = entry);
            (array, memory_map_len)
        };
        let memory_map = &(memory_map[..memory_map_len]);

        let total_memory = {
            let last_entry = memory_map.last()?;
            ((last_entry.base + last_entry.len) & !0xFFF) as usize
        };
        let total_frames = total_memory / 0x1000;

        let table_size_in_bytes = libcommon::align_up(
            total_frames * core::mem::size_of::<FrameData>(),
            // ### Safety: Value provided is non-zero.
            unsafe { NonZeroUsize::new_unchecked(0x1000) },
        );
        let table_entry =
            memory_map.iter().find(|entry| entry.typ == FrameType::Generic && entry.len >= table_size_in_bytes)?;
        let table = unsafe {
            core::slice::from_raw_parts(
                physical_memory.as_ptr().add(table_entry.base as usize).cast::<FrameData>(),
                total_frames,
            )
        };

        memory_map
            .iter()
            .map(|entry| (entry.base / 0x1000, entry.len / 0x1000, entry.typ))
            .flat_map(|(base_index, count, typ)| (base_index..(base_index + count)).map(move |index| (index, typ)))
            .for_each(|(index, typ)| {
                let frame_data = &table[index];
                frame_data.peek();
                frame_data.set_type(typ);
                frame_data.unpeek();
            });

        // Ensure the table pages are reserved, so as to not be locked by any of the `_next` functions.
        table.iter().skip((table_entry.base / 0x1000) as usize).take(table_size_in_bytes / 0x1000).for_each(
            |frame_data| {
                frame_data.peek();
                frame_data.set_type(FrameType::Reserved);
                frame_data.unpeek();
            },
        );

        Some(Self { table, physical_memory })
    }

    #[inline]
    pub const fn total_memory(&self) -> usize {
        self.table.len() * 0x1000
    }

    #[inline]
    fn with_table<T>(&self, func: impl FnOnce(&[FrameData]) -> T) -> T {
        crate::interrupts::without(|| func(self.table))
    }

    pub fn next_frame(&self) -> Result<Address<Frame>> {
        self.with_table(|table| {
            table
                .iter()
                .enumerate()
                .find_map(|(index, frame_data)| {
                    frame_data.peek();

                    if let (false, FrameType::Generic) = frame_data.data() {
                        frame_data.lock();
                        frame_data.unpeek();

                        Some((index * 0x1000) as u64)
                    } else {
                        frame_data.unpeek();

                        None
                    }
                })
                .and_then(Address::<Frame>::from_u64)
                .ok_or(Error::NoneFree)
        })
    }

    pub fn next_frames(&self, count: NonZeroUsize, alignment: NonZeroUsize) -> Result<Address<Frame>> {
        if !alignment.is_power_of_two() {
            return Err(Error::InvalidAlignment);
        }

        self.with_table(|mut sub_table| {
            let alignment = core::cmp::max(alignment.get() / 0x1000, 1);

            // Loop the table and attempt to locate a viable range of blocks.
            loop {
                let pages = sub_table.iter().take(count.get());
                if pages.len() < count.get() {
                    break Err(Error::NoneFree);
                }

                let pages = &sub_table[..count.get()];
                pages.iter().for_each(FrameData::peek);

                match pages
                    .iter()
                    .map(FrameData::data)
                    .enumerate()
                    .rfind(|(_, (locked, ty))| *locked || *ty != FrameType::Generic)
                {
                    Some((index, _)) => {
                        pages.iter().for_each(FrameData::unpeek);
                        let aligned_index = ((index + 1) + (alignment - 1)) / alignment;
                        sub_table = &sub_table[aligned_index..];
                    }

                    None => {
                        pages.iter().for_each(|frame_data| {
                            frame_data.lock();
                            frame_data.unpeek();
                        });

                        // Use wrapping arithmetic here to make any errors in computation painfully obvious due
                        // to extremely unpredictable results.
                        let start_index = self.table.len().wrapping_sub(sub_table.len());
                        let start_address = start_index.wrapping_mul(0x1000);
                        break Address::<Frame>::from_u64(start_address as u64).ok_or(Error::Unknown);
                    }
                }
            }
        })
    }

    pub fn lock_frame(&self, frame: Address<Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame_data) = table.get(frame.index()) else { return Err(Error::OutOfBounds) };
            frame_data.peek();

            let (locked, _) = frame_data.data();
            if !locked {
                frame_data.lock();
                frame_data.unpeek();

                Ok(())
            } else {
                frame_data.unpeek();

                Err(Error::NotFree)
            }
        })
    }

    pub fn lock_frames(&self, base: Address<Frame>, count: usize) -> Result<()> {
        self.with_table(|table| {
            let start_index = base.index();
            let end_index = start_index + count;
            if (base.index() + count) > table.len() {
                return Err(Error::OutOfBounds);
            }

            let table = &table[start_index..end_index];

            table.iter().for_each(FrameData::peek);

            if table.iter().map(FrameData::data).all(|(locked, _)| !locked) {
                table.iter().for_each(|frame_data| {
                    frame_data.lock();
                    frame_data.unpeek();
                });

                Ok(())
            } else {
                table.iter().for_each(FrameData::unpeek);

                Err(Error::NotFree)
            }
        })
    }

    pub fn free_frame(&self, frame: Address<libcommon::Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame_data) = table.get(frame.index()) else { return Err(Error::OutOfBounds) };

            frame_data.peek();

            match frame_data.data() {
                (locked, _) if locked => {
                    frame_data.free();
                    frame_data.unpeek();

                    Ok(())
                }

                _ => {
                    frame_data.unpeek();

                    Err(Error::NotLocked)
                }
            }
        })
    }
}

unsafe impl core::alloc::Allocator for &PhysicalMemoryManager<'_> {
    fn allocate(&self, layout: core::alloc::Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        let layout = layout.align_to(0x1000).map_err(|_| AllocError)?.pad_to_align();
        let physical_memory = self.physical_memory;
        self.next_frames(
            NonZeroUsize::new(layout.size() / 0x1000).unwrap(),
            NonZeroUsize::new(layout.align() / 0x1000).unwrap_or(NonZeroUsize::MIN),
        )
        .ok()
        .and_then(|address| {
            physical_memory
                .addr()
                .checked_add(address.as_usize())
                .map(|address| NonNull::slice_from_raw_parts(physical_memory.with_addr(address), layout.size()))
        })
        .ok_or(AllocError)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        debug_assert!(ptr.as_ptr().is_aligned_to(0x1000));

        let Ok(layout) = layout.align_to(0x1000).map(|layout| layout.pad_to_align())
            else {
                error!("Unexpectedly failed to align deallocation layout.");
                return;
            };

        let base_address = ptr.addr().get() - self.physical_memory.addr().get();
        for offset in (0..layout.size()).step_by(0x1000) {
            Address::<Frame>::from_u64((base_address + offset) as u64)
                .and_then(|address| self.free_frame(address).ok())
                .unwrap_or_else(|| {
                    error!("Unexpectedly failed to free frame during deallocation");
                });
        }
    }
}
