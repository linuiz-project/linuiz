use alloc::{alloc::Global, vec::Vec};
use bit_field::BitField;
use core::{
    alloc::AllocError,
    num::NonZeroUsize,
    sync::atomic::{AtomicU8, Ordering},
};
use libcommon::{
    memory::{page_aligned_allocator, AlignedAllocator, KernelAllocator},
    Address, Virtual,
};
use spin::Mutex;

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
pub struct Frame(AtomicU8);

impl Frame {
    const LOCKED_SHIFT: usize = 7;
    const PEEKED_SHIFT: usize = 6;
    const LOCKED_BIT: u8 = 1 << Self::LOCKED_SHIFT;
    const PEEKED_BIT: u8 = 1 << Self::PEEKED_SHIFT;
    const TYPE_RANGE: core::ops::Range<usize> = 0..4;

    #[inline(always)]
    fn lock(&self) {
        let lock_result = self.0.fetch_or(Self::LOCKED_BIT, Ordering::AcqRel);
        debug_assert!(!lock_result.get_bit(Self::LOCKED_SHIFT));
    }

    #[inline(always)]
    fn free(&self) {
        let free_result = self.0.fetch_xor(Self::LOCKED_BIT, Ordering::AcqRel);
        debug_assert!(free_result.get_bit(Self::LOCKED_SHIFT));
    }

    #[inline(always)]
    fn try_peek(&self) -> bool {
        !self.0.fetch_or(Self::PEEKED_BIT, Ordering::AcqRel).get_bit(Self::PEEKED_SHIFT)
    }

    #[inline(always)]
    fn peek(&self) {
        while !self.try_peek() {
            core::hint::spin_loop();
        }
    }

    #[inline(always)]
    fn unpeek(&self) {
        let unpeek_result = self.0.fetch_and(!Self::PEEKED_BIT, Ordering::AcqRel);
        debug_assert!(unpeek_result.get_bit(Self::PEEKED_SHIFT));
    }

    #[inline(always)]
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

type Result<T> = core::result::Result<T, AllocError>;

pub struct SlabAllocator<'a> {
    slabs64: Mutex<Vec<(*mut u8, u64), AlignedAllocator<0x1000, Global>>>,
    slabs128: Mutex<Vec<(*mut u8, u32), AlignedAllocator<0x1000, Global>>>,
    slabs256: Mutex<Vec<(*mut u8, u16), AlignedAllocator<0x1000, Global>>>,
    slabs512: Mutex<Vec<(*mut u8, u8), AlignedAllocator<0x1000, Global>>>,
    phys_mapped_address: Address<Virtual>,
    table: &'a [Frame],
}

// SAFETY: Type uses a global physical mapped address, and so is thread-independent.
unsafe impl Send for SlabAllocator<'_> {}
// SAFETY: Type ensures all concurrent accesses are synchronized.
unsafe impl Sync for SlabAllocator<'_> {}

impl<'a> SlabAllocator<'a> {
    // SAFETY: Caller must guarantee the physical mapped address is valid.
    pub unsafe fn from_memory_map(
        memory_map: &[crate::MmapEntry],
        phys_mapped_address: Address<Virtual>,
    ) -> Option<Self> {
        let page_count = libcommon::align_up_div(
            memory_map.last().map(|entry| entry.base + entry.len).unwrap() as usize,
            // SAFETY: Value provided is non-zero.
            unsafe { NonZeroUsize::new_unchecked(0x1000) },
        );
        let table_bytes = libcommon::align_up(
            page_count * core::mem::size_of::<Frame>(),
            // SAFETY: Value provided is non-zero.
            unsafe { NonZeroUsize::new_unchecked(0x1000) },
        );

        let table_entry = memory_map
            .iter()
            .filter(|entry| entry.typ == limine::LimineMemoryMapEntryType::Usable)
            .find(|entry| entry.len >= (table_bytes as u64))?;

        // Clear the memory of the chosen region.
        unsafe { core::ptr::write_bytes(table_entry.base as *mut u8, 0, table_bytes) };

        let table = unsafe {
            core::slice::from_raw_parts((phys_mapped_address.as_u64() + table_entry.base) as *mut Frame, page_count)
        };

        for entry in memory_map {
            assert_eq!(entry.base & 0xFFF, 0, "memory map entry is not page-aligned: {entry:?}");

            let base_index = entry.base / 0x1000;
            let count = entry.len / 0x1000;

            // Translate UEFI memory type to kernel frame type.
            let frame_ty = {
                use crate::MmapEntryType;

                match entry.typ {
                    MmapEntryType::Usable => FrameType::Generic,
                    MmapEntryType::BootloaderReclaimable => FrameType::BootReclaim,
                    MmapEntryType::AcpiReclaimable => FrameType::AcpiReclaim,
                    MmapEntryType::KernelAndModules
                    | MmapEntryType::Reserved
                    | MmapEntryType::AcpiNvs
                    | MmapEntryType::Framebuffer => FrameType::Reserved,
                    MmapEntryType::BadMemory => FrameType::Unusable,
                }
            };

            (base_index..(base_index + count)).map(|index| &table[index as usize]).for_each(|frame| {
                frame.peek();
                frame.set_type(frame_ty);
                frame.unpeek();
            });
        }

        // Ensure the table pages are reserved, so as to not be locked by any of the `_next` functions.
        table
            .iter()
            .skip((table_entry.base / 0x1000) as usize)
            .take(libcommon::align_up_div(
                table_bytes,
                // SAFETY: Value provided is non-zero.
                unsafe { NonZeroUsize::new_unchecked(0x1000) },
            ))
            .for_each(|frame| {
                frame.peek();
                frame.set_type(FrameType::Reserved);
                frame.unpeek();
            });

        Some(Self {
            slabs64: Mutex::new(Vec::new_in(page_aligned_allocator())),
            slabs128: Mutex::new(Vec::new_in(page_aligned_allocator())),
            slabs256: Mutex::new(Vec::new_in(page_aligned_allocator())),
            slabs512: Mutex::new(Vec::new_in(page_aligned_allocator())),
            phys_mapped_address,
            table,
        })
    }

    fn with_table<T>(&self, func: impl FnOnce(&[Frame]) -> T) -> T {
        libarch::interrupts::without(|| func(self.table))
    }
}

macro_rules! slab_allocate {
    ($self:expr, $slabs_name:ident, $slab_size:expr) => {
        {
            let mut slabs = $self.$slabs_name.lock();
            let allocation_ptr =
                match slabs.iter_mut().find(|(_, allocations)| allocations.trailing_zeros() > 0) {
                    Some((memory_ptr, allocations)) => {
                        let allocation_bit = (allocations.trailing_zeros() - 1) as usize;
                        allocations.set_bit(allocation_bit, true);

                        // SAFETY: Arbitrary `u8` memory region is valid for the offsets within its bounds.
                        unsafe { memory_ptr.add(allocation_bit * $slab_size) }
                    }

                    None if let Ok(frame) = $self.lock_next() => {
                        // SAFETY: `phys_mapped_address` is required to be valid for arbitrary offsets from within its range.
                        let memory_ptr = unsafe { $self.phys_mapped_address.as_mut_ptr::<u8>().add(frame.as_usize()) };
                        slabs.push((memory_ptr, 1 << ((0x1000 / $slab_size) - 1)));

                        memory_ptr
                    }

                    None => unimplemented!()
                };

            Ok(slice_from_raw_parts_mut(allocation_ptr, $slab_size))
        }
    };
}

macro_rules! slab_deallocate {
    ($self:expr, $slabs_name:ident, $slab_size:expr, $ptr:expr) => {
        let ptr_addr = $ptr.addr().get();
        let mut slabs = $self.$slabs_name.lock();

        for (memory_ptr, allocations) in slabs.iter_mut() {
            let memory_range = memory_ptr.addr()..(memory_ptr.addr() + 4096);
            if memory_range.contains(&ptr_addr) {
                let allocation_offset = ptr_addr - memory_range.start;
                let allocation_bit = allocation_offset / $slab_size;
                allocations.set_bit(allocation_bit, false);
            }
        }
    };
}

// SAFETY: `SlabAllocator` promises to do everything right.
unsafe impl<'a> core::alloc::Allocator for SlabAllocator<'a> {
    fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>> {
        use core::ptr::{slice_from_raw_parts_mut, NonNull};

        let allocation_ptr = {
            if layout.align() <= 64 && layout.size() <= 64 {
                slab_allocate!(self, slabs64, 64)
            } else if layout.align() <= 128 && layout.size() <= 128 {
                slab_allocate!(self, slabs128, 128)
            } else if layout.align() <= 256 && layout.size() <= 246 {
                slab_allocate!(self, slabs256, 256)
            } else if layout.align() <= 512 && layout.size() <= 512 {
                slab_allocate!(self, slabs512, 512)
            } else {
                (if layout.align() <= 4096 && layout.size() <= 4096 {
                    self.lock_next()
                } else {
                    self.lock_next_many(
                        NonZeroUsize::new(layout.size() / 0x1000).unwrap(),
                        // SAFETY: `Layout::align()` can not be zero in safe Rust.
                        unsafe { NonZeroUsize::new_unchecked(layout.align()) },
                    )
                })
                .map(|address| {
                    // SAFETY: Frame addresses are naturally aligned, and arbitrary memory is valid for `u8`, and `phys_mapped_address` is
                    //         required to be valid for arbitrary offsets from within its range.
                    let allocation_ptr = unsafe { self.phys_mapped_address.as_mut_ptr::<u8>().add(address.as_usize()) };
                    slice_from_raw_parts_mut(
                        allocation_ptr,
                        libcommon::align_up(
                            layout.size(),
                            // SAFETY: Value provided is non-zero.
                            unsafe { NonZeroUsize::new_unchecked(0x1000) },
                        ),
                    )
                })
            }
        };

        match allocation_ptr {
            Ok(allocation_slice) if let Some(non_null) = NonNull::new(allocation_slice) => Ok(non_null),
            _ => Err(AllocError),
        }
    }

    unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
        if layout.align() <= 64 && layout.size() <= 64 {
            slab_deallocate!(self, slabs64, 64, ptr);
        } else if layout.align() <= 128 && layout.size() <= 128 {
            slab_deallocate!(self, slabs128, 128, ptr);
        } else if layout.align() <= 256 && layout.size() <= 246 {
            slab_deallocate!(self, slabs256, 256, ptr);
        } else if layout.align() <= 512 && layout.size() <= 512 {
            slab_deallocate!(self, slabs512, 512, ptr);
        } else {
            todo!("don't leak memory")
        }
    }
}

impl KernelAllocator for SlabAllocator<'_> {
    fn lock_next(&self) -> Result<Address<libcommon::Frame>> {
        self.with_table(|table| {
            table
                .iter()
                .enumerate()
                .find_map(|(index, frame)| {
                    frame.peek();

                    if let (false, FrameType::Generic) = frame.data() {
                        frame.lock();
                        frame.unpeek();

                        Some(Address::<libcommon::Frame>::new_truncate((index * 0x1000) as u64))
                    } else {
                        frame.unpeek();

                        None
                    }
                })
                .ok_or(AllocError)
        })
    }

    fn lock_next_many(&self, count: NonZeroUsize, alignment: NonZeroUsize) -> Result<Address<libcommon::Frame>> {
        assert!(alignment.is_power_of_two());

        self.with_table(|table| {
            assert!(count.get() < table.len());

            let alignment = core::cmp::max(alignment.get() / 0x1000, 1);
            let mut sub_table = table;
            while !sub_table.is_empty() {
                if sub_table.len() < count.get() {
                    return Err(AllocError);
                }

                let frames = &sub_table[..count.get()];
                frames.iter().for_each(Frame::peek);

                match frames
                    .iter()
                    .map(Frame::data)
                    .enumerate()
                    .rfind(|(_, (locked, ty))| !locked || *ty != FrameType::Generic)
                {
                    Some((index, _)) => {
                        frames.iter().for_each(Frame::unpeek);
                        sub_table = &sub_table[libcommon::align_up(
                            index,
                            // SAFETY: Value (via `max(alignment / 0x1000, 1)`) is guaranteed to be >0.
                            unsafe { NonZeroUsize::new_unchecked(alignment) },
                        )..]
                    }

                    None => {
                        frames.iter().for_each(Frame::lock);
                        frames.iter().for_each(Frame::unpeek);
                        return Ok(Address::<libcommon::Frame>::new_truncate(
                            ((table.len() - sub_table.len()) * 0x1000) as u64,
                        ));
                    }
                }
            }

            Err(AllocError)
        })
    }

    fn lock(&self, frame: Address<libcommon::Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame) = table.get(frame.index()) else { return Err(AllocError) };
            frame.peek();

            let (locked, _) = frame.data();
            if !locked {
                frame.lock();
                frame.unpeek();

                Ok(())
            } else {
                frame.unpeek();

                Err(AllocError)
            }
        })
    }

    fn lock_many(&self, base: Address<libcommon::Frame>, count: usize) -> Result<()> {
        self.with_table(|table| {
            let frames = &table[base.index()..(base.index() + count)];
            frames.iter().for_each(Frame::peek);

            if frames.iter().map(Frame::data).all(|(locked, _)| !locked) {
                frames.iter().for_each(Frame::lock);
                frames.iter().for_each(Frame::unpeek);

                Ok(())
            } else {
                frames.iter().for_each(Frame::unpeek);

                Err(AllocError)
            }
        })
    }

    fn free(&self, frame: Address<libcommon::Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame) = table.get(frame.index()) else { return Err(AllocError) };

            frame.peek();

            match frame.data() {
                (locked, _) if locked => {
                    frame.free();
                    frame.unpeek();

                    Ok(())
                }

                _ => {
                    frame.unpeek();

                    Err(AllocError)
                }
            }
        })
    }

    // TODO non-zero usize for the count
    fn allocate_to(&self, frame: Address<libcommon::Frame>, count: usize) -> Result<Address<Virtual>> {
        self.lock_many(frame, count)
            .map(|_| Address::<Virtual>::new_truncate(self.phys_mapped_address.as_u64() + frame.as_u64()))
    }

    fn total_memory(&self) -> usize {
        self.table.len() * 0x1000
    }
}
