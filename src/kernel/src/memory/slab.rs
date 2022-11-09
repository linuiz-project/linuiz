use bit_field::BitField;
use core::{
    alloc::AllocError,
    num::NonZeroUsize,
    ptr::{slice_from_raw_parts_mut, NonNull},
    sync::atomic::{AtomicU8, Ordering},
};
use libcommon::{Address, Frame, Virtual};
use lzalloc::{vec::Vec, AlignedAllocator, AllocResult};
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
pub struct FrameData(AtomicU8);

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

const MIN_SLAB_SIZE_SHIFT: usize = 6;
const MIN_SLAB_SIZE: usize = 1 << MIN_SLAB_SIZE_SHIFT;

const SLAB_GROUPS: usize = 4;
const SLAB_INDEXES: usize = 0x1000 / MIN_SLAB_SIZE;

const MAX_SLAB_SIZE_SHIFT: usize = (SLAB_GROUPS - 1) + MIN_SLAB_SIZE_SHIFT;
const MAX_SLAB_SIZE: usize = 1 << MAX_SLAB_SIZE_SHIFT;

pub struct SlabAllocator<'a> {
    slabs: [Mutex<Vec<(NonNull<[u8]>, bitvec::BitArr!(for SLAB_INDEXES)), AlignedAllocator<0x1000>>>; SLAB_GROUPS],
    phys_mapped_address: Address<Virtual>,
    table: &'a [FrameData],
}

// ### Safety: Type uses a global physical mapped address, and so is thread-independent.
unsafe impl Send for SlabAllocator<'_> {}
// ### Safety: Type ensures all concurrent accesses are synchronized.
unsafe impl Sync for SlabAllocator<'_> {}

impl SlabAllocator<'_> {
    // ### Safety: Caller must guarantee the physical mapped address is valid.
    pub unsafe fn from_memory_map(
        memory_map: &[crate::MmapEntry],
        phys_mapped_address: Address<Virtual>,
    ) -> Option<Self> {
        let page_count = libcommon::align_up_div(
            memory_map.last().map(|entry| entry.base + entry.len).unwrap() as usize,
            // ### Safety: Value provided is non-zero.
            unsafe { NonZeroUsize::new_unchecked(0x1000) },
        );
        let table_bytes = libcommon::align_up(
            page_count * core::mem::size_of::<FrameData>(),
            // ### Safety: Value provided is non-zero.
            unsafe { NonZeroUsize::new_unchecked(0x1000) },
        );

        let table_entry = memory_map
            .iter()
            .filter(|entry| entry.typ == limine::LimineMemoryMapEntryType::Usable)
            .find(|entry| entry.len >= (table_bytes as u64))?;

        // Clear the memory of the chosen region.
        unsafe { core::ptr::write_bytes(table_entry.base as *mut u8, 0, table_bytes) };

        let table = unsafe {
            core::slice::from_raw_parts((phys_mapped_address.as_u64() + table_entry.base) as *mut FrameData, page_count)
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

            (base_index..(base_index + count)).map(|index| &table[index as usize]).for_each(|frame_data| {
                frame_data.peek();
                frame_data.set_type(frame_ty);
                frame_data.unpeek();
            });
        }

        // Ensure the table pages are reserved, so as to not be locked by any of the `_next` functions.
        table
            .iter()
            .skip((table_entry.base / 0x1000) as usize)
            .take(libcommon::align_up_div(
                table_bytes,
                // ### Safety: Value provided is non-zero.
                unsafe { NonZeroUsize::new_unchecked(0x1000) },
            ))
            .for_each(|frame_data| {
                frame_data.peek();
                frame_data.set_type(FrameType::Reserved);
                frame_data.unpeek();
            });

        Some(Self {
            slabs: [const { Mutex::new(Vec::new_in(AlignedAllocator)) }; SLAB_GROUPS],
            phys_mapped_address,
            table,
        })
    }

    #[inline]
    pub const fn total_memory(&self) -> usize {
        self.table.len() * 0x1000
    }

    #[inline]
    fn with_table<T>(&self, func: impl FnOnce(&[FrameData]) -> T) -> T {
        crate::interrupts::without(|| func(self.table))
    }

    pub fn next_frame(&self) -> AllocResult<Address<Frame>> {
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
                .ok_or(AllocError)
        })
    }

    pub fn next_frames(&self, count: NonZeroUsize, alignment: NonZeroUsize) -> AllocResult<Address<Frame>> {
        if !alignment.is_power_of_two() {
            return Err(AllocError);
        }

        self.with_table(|mut sub_table| {
            let alignment = core::cmp::max(alignment.get() / 0x1000, 1);
            // Loop the table and attempt to locate a viable range of blocks.
            (loop {
                if sub_table.len() < count.get() {
                    break None;
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
                        pages.iter().for_each(FrameData::lock);
                        pages.iter().for_each(FrameData::unpeek);

                        // Use wrapping arithmetic here to make any errors in computation painfully obvious due
                        // to extremely unpredictable results.
                        break Some(self.table.len().wrapping_sub(sub_table.len()).wrapping_mul(0x1000) as u64);
                    }
                }
            })
            .and_then(Address::<Frame>::from_u64)
            .ok_or(AllocError)
        })
    }

    pub fn lock_frame(&self, frame: Address<libcommon::Frame>) -> AllocResult<()> {
        self.with_table(|table| {
            let Some(frame_data) = table.get(frame.index()) else { return Err(AllocError) };
            frame_data.peek();

            let (locked, _) = frame_data.data();
            if !locked {
                frame_data.lock();
                frame_data.unpeek();

                Ok(())
            } else {
                frame_data.unpeek();

                Err(AllocError)
            }
        })
    }

    pub fn lock_frames(&self, base: Address<libcommon::Frame>, count: usize) -> AllocResult<()> {
        self.with_table(|table| {
            let frames = &table[base.index()..(base.index() + count)];
            frames.iter().for_each(FrameData::peek);

            if frames.iter().map(FrameData::data).all(|(locked, _)| !locked) {
                frames.iter().for_each(FrameData::lock);
                frames.iter().for_each(FrameData::unpeek);

                Ok(())
            } else {
                frames.iter().for_each(FrameData::unpeek);

                Err(AllocError)
            }
        })
    }

    pub fn free_frame(&self, frame: Address<libcommon::Frame>) -> AllocResult<()> {
        self.with_table(|table| {
            let Some(frame_data) = table.get(frame.index()) else { return Err(AllocError) };

            frame_data.peek();

            match frame_data.data() {
                (locked, _) if locked => {
                    frame_data.free();
                    frame_data.unpeek();

                    Ok(())
                }

                _ => {
                    frame_data.unpeek();

                    Err(AllocError)
                }
            }
        })
    }

    // TODO non-zero usize for the count
    pub fn allocate_to(&self, frame: Address<libcommon::Frame>, count: usize) -> AllocResult<Address<Virtual>> {
        self.lock_frames(frame, count)
            .map(|_| Address::<Virtual>::new_truncate(self.phys_mapped_address.as_u64() + frame.as_u64()))
    }
}

// ### Safety: `SlabAllocator` promises to do everything right.
unsafe impl<'a> core::alloc::Allocator for SlabAllocator<'a> {
    fn allocate(&self, layout: core::alloc::Layout) -> AllocResult<NonNull<[u8]>> {
        debug_assert!(!crate::interrupts::are_enabled());

        if layout.size() <= MAX_SLAB_SIZE && layout.align() <= MAX_SLAB_SIZE {
            let slab_size = match core::cmp::max(layout.size(), layout.align()) {
                value if value.is_power_of_two() => value,
                value => value.next_power_of_two(),
            };
            let slab_index = (core::cmp::max(slab_size, MIN_SLAB_SIZE).trailing_zeros() as usize) - MIN_SLAB_SIZE_SHIFT;

            let mut slabs = self.slabs.get(slab_index).ok_or(AllocError)?.lock();
            match slabs.iter_mut().find(|(_, bits)| bits.not_all()) {
                Some((memory_ptr, bits)) => {
                    // ### Safety: BitArray is known to contain zeroes.
                    let index = unsafe { bits.first_zero().unwrap_unchecked() };
                    bits.set(index, true);
                    Ok(unsafe {
                        NonNull::new_unchecked(slice_from_raw_parts_mut(
                            memory_ptr.as_non_null_ptr().as_ptr().add(index * slab_size),
                            slab_size,
                        ))
                    })
                }

                None => {
                    let new_frame_data =
                        self.next_frames(unsafe { NonZeroUsize::new_unchecked(1 << slab_index) }, NonZeroUsize::MIN)?;
                    // ### Safety: `phys_mapped_address` is required to be valid for arbitrary offsets from within its range.
                    let memory_ptr = unsafe {
                        NonNull::new_unchecked(slice_from_raw_parts_mut(
                            self.phys_mapped_address.as_mut_ptr::<u8>().with_addr(new_frame_data.as_usize()),
                            slab_size * SLAB_INDEXES,
                        ))
                    };
                    let mut bit_array = bitvec::bitarr![0; SLAB_INDEXES];
                    bit_array.set(0, true);
                    slabs.push((memory_ptr, bit_array)).map_err(|_| AllocError)?;

                    Ok(memory_ptr)
                }
            }
        } else if layout.size() <= 0x1000 && layout.align() <= 0x1000 {
            // ... frame-sized allocation ...

            self.next_frame().map(|address| unsafe {
                NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                    // ### Safety: Physical mapping address is valid for its memory region.
                    self.phys_mapped_address.as_mut_ptr::<u8>().add(address.as_usize()),
                    0x1000,
                ))
            })
        } else {
            // ... many frames-sized allocation ...

            let frame_count = (layout.size() + 0xFFF) / 0x1000;
            self.next_frames(
                // ### Safety: Valid `Layout`s do not allow `.size()` to be 0.
                unsafe { NonZeroUsize::new_unchecked(frame_count) },
                // ### Safety: Valid `Layout`s do not allow `.align()` to be 0.
                unsafe { NonZeroUsize::new_unchecked(layout.align()) },
            )
            .map(|address| unsafe {
                NonNull::new_unchecked(core::ptr::slice_from_raw_parts_mut(
                    // ### Safety: Physical mapping address is valid for its memory region.
                    self.phys_mapped_address.as_mut_ptr::<u8>().add(address.as_usize()),
                    frame_count * 0x1000,
                ))
            })
        }
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: core::alloc::Layout) {
        todo!("don't leak memory")
    }
}
