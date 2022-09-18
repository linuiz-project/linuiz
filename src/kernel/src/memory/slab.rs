use alloc::{alloc::Global, vec::Vec};
use bit_field::BitField;
use core::{alloc::AllocError, sync::atomic::Ordering};
use libkernel::{
    memory::{page_aligned_allocator, AlignedAllocator},
    Address, Virtual,
};
use spin::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    Unusable,
    Generic,
    Reserved,
    BootReclaim,
    AcpiReclaim,
}

impl FrameType {
    fn from_u16(value: u16) -> Self {
        match value {
            0 => Self::Unusable,
            1 => Self::Generic,
            2 => Self::Reserved,
            3 => Self::BootReclaim,
            4 => Self::AcpiReclaim,
            _ => unimplemented!(),
        }
    }

    fn as_u16(self) -> u16 {
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
#[repr(transparent)]
struct Frame(core::sync::atomic::AtomicU16);

impl Frame {
    const PEEKED_BIT_SHIFT: usize = 12;
    const PEEKED_BIT: u16 = 1 << Self::PEEKED_BIT_SHIFT;
    const TYPE_SHIFT: u16 = 13;
    const REFERENCE_COUNT_MASK: u16 = 0xFFF;

    fn borrow(&self) {
        let old_value = self.0.fetch_add(1, Ordering::Relaxed);
        debug_assert_ne!(old_value, Self::REFERENCE_COUNT_MASK, "reference count overflow");
    }

    fn free(&self) {
        let old_value = self.0.fetch_sub(1, Ordering::Relaxed);
        debug_assert_ne!(old_value, 0, "reference count underflow");
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

        debug_assert!(self.0.load(Ordering::Relaxed).get_bit(Self::PEEKED_BIT_SHIFT));
    }

    #[inline]
    fn unpeek(&self) {
        debug_assert!(self.0.load(Ordering::Relaxed).get_bit(Self::PEEKED_BIT_SHIFT));
        self.0.fetch_and(!Self::PEEKED_BIT, Ordering::Relaxed);
    }

    /// Returns the frame data in a tuple.
    fn data(&self) -> (u16, FrameType) {
        debug_assert!(self.0.load(Ordering::Relaxed).get_bit(Self::PEEKED_BIT_SHIFT));

        let raw = self.0.load(Ordering::Relaxed);

        (raw & Self::REFERENCE_COUNT_MASK, FrameType::from_u16(raw >> Self::TYPE_SHIFT))
    }

    fn modify_type(&self, new_type: FrameType) {
        debug_assert!(self.0.load(Ordering::Relaxed).get_bit(Self::PEEKED_BIT_SHIFT));

        self.0.store(
            (self.0.load(Ordering::Relaxed) & !(u16::MAX >> Self::TYPE_SHIFT))
                | (new_type.as_u16() << Self::TYPE_SHIFT),
            Ordering::Relaxed,
        );
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
    pub fn from_memory_map(
        memory_map: &[limine::NonNullPtr<limine::LimineMemmapEntry>],
        phys_mapped_address: Address<Virtual>,
    ) -> Option<Self> {
        let page_count =
            libkernel::align_up_div(memory_map.last().map(|entry| entry.base + entry.len).unwrap() as usize, 0x1000);
        let table_bytes = libkernel::align_up(page_count * core::mem::size_of::<Frame>(), 0x1000);

        let table_entry = memory_map
            .iter()
            .filter(|entry| entry.typ == limine::LimineMemoryMapEntryType::Usable)
            .find(|entry| entry.len >= (table_bytes as u64))?;

        // Clear the memory of the chosen region.
        unsafe { core::ptr::write_bytes(table_entry.base as *mut u8, 0, table_entry.len as usize) };

        let table = unsafe {
            core::slice::from_raw_parts((phys_mapped_address.as_u64() + table_entry.base) as *mut Frame, page_count)
        };

        table.iter().skip((table_entry.base / 0x1000) as usize).take((table_entry.len / 0x1000) as usize).for_each(
            |table_page| {
                table_page.peek();

                table_page.borrow();
                table_page.modify_type(FrameType::Reserved);

                table_page.unpeek();
            },
        );

        for entry in memory_map {
            assert_eq!(entry.base & 0xFFF, 0, "memory map entry is not page-aligned: {entry:?}");

            let base_index = entry.base / 0x1000;
            let count = entry.len / 0x1000;

            // Translate UEFI memory type to kernel frame type.
            let frame_ty = {
                use limine::LimineMemoryMapEntryType;

                match entry.typ {
                    LimineMemoryMapEntryType::Usable
                    | LimineMemoryMapEntryType::AcpiNvs
                    | LimineMemoryMapEntryType::Framebuffer => FrameType::Generic,
                    LimineMemoryMapEntryType::BootloaderReclaimable => FrameType::BootReclaim,
                    LimineMemoryMapEntryType::AcpiReclaimable => FrameType::AcpiReclaim,
                    LimineMemoryMapEntryType::KernelAndModules | LimineMemoryMapEntryType::Reserved => {
                        FrameType::Reserved
                    }
                    LimineMemoryMapEntryType::BadMemory => FrameType::Unusable,
                }
            };

            (base_index..(base_index + count)).map(|index| &table[index as usize]).for_each(|frame| {
                frame.peek();
                frame.modify_type(frame_ty);
                frame.unpeek();
            });
        }

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
        crate::interrupts::without(|| func(self.table))
    }

    pub fn lock_next(&self) -> Result<Address<libkernel::Frame>> {
        self.with_table(|table| {
            table
                .iter()
                .enumerate()
                .find_map(|(index, table_page)| {
                    if table_page.try_peek() {
                        let (ref_count, ty) = table_page.data();

                        let result = if ref_count == 0 && ty == FrameType::Generic {
                            table_page.borrow();
                            Some(Address::<libkernel::Frame>::new_truncate((index * 0x1000) as u64))
                        } else {
                            None
                        };

                        table_page.unpeek();
                        result
                    } else {
                        None
                    }
                })
                .ok_or(AllocError)
        })
    }

    pub fn lock_next_many(&self, count: usize) -> Result<Address<libkernel::Frame>> {
        self.with_table(|table| {
            let mut start_index = 0;

            while start_index < (table.len() - count) {
                let sub_table = &table[start_index..(start_index + count)];
                sub_table.iter().for_each(Frame::peek);

                match sub_table
                    .iter()
                    .enumerate()
                    .map(|(index, frame)| (index, frame.data()))
                    .find(|(_, (ref_count, ty))| *ref_count > 0 || *ty != FrameType::Generic)
                {
                    Some((index, _)) => {
                        start_index += index + 1;
                        sub_table.iter().for_each(Frame::unpeek);
                    }
                    None => {
                        sub_table.iter().for_each(|frame| {
                            frame.borrow();
                            frame.unpeek();
                        });

                        return Ok(Address::<libkernel::Frame>::new_truncate((start_index * 0x1000) as u64));
                    }
                }
            }

            Err(AllocError)
        })
    }

    pub fn borrow(&self, frame: Address<libkernel::Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame) = table.get(frame.index()) else { return Err(AllocError) };
            frame.peek();

            let (ref_count, ty) = frame.data();
            if ref_count < 0xFFF && ty == FrameType::Generic {
                frame.borrow();
                frame.unpeek();

                Ok(())
            } else {
                frame.unpeek();

                Err(AllocError)
            }
        })
    }

    pub fn borrow_many(&self, base: Address<libkernel::Frame>, count: usize) -> Result<()> {
        self.with_table(|table| {
            let frames = &table[base.index()..(base.index() + count)];

            frames.iter().for_each(Frame::peek);
            if frames.iter().map(Frame::data).all(|(ref_count, ty)| ref_count < 0xFFF && ty == FrameType::Generic) {
                frames.iter().for_each(Frame::borrow);
                frames.iter().for_each(Frame::unpeek);

                Ok(())
            } else {
                frames.iter().for_each(Frame::unpeek);

                Err(AllocError)
            }
        })
    }

    pub fn free(&self, frame: Address<libkernel::Frame>) -> Result<()> {
        self.with_table(|table| {
            let Some(frame) = table.get(frame.index()) else { return Err(AllocError) };

            frame.peek();

            match frame.data() {
                (ref_count, _) if ref_count > 0 => {
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
}

macro_rules! slab_allocate {
    ($self:expr, $slabs_name:ident, $slab_size:expr) => {
        {
            let mut slabs = $self.$slabs_name.lock();
            let mut allocation_ptr_cell = core::cell::OnceCell::new();
            while let None = allocation_ptr_cell.get() {
                match slabs.iter_mut().find_map(|(memory_ptr, allocations)| {
                    let allocations_remaining = allocations.trailing_zeros();

                    if allocations_remaining > 0 {
                        let allocation_bit = (allocations_remaining - 1) as usize;
                        allocations.set_bit(allocation_bit, true);
                        // SAFETY: Arbitrary `u8` memory region is valid for any offset.
                        Some(unsafe { memory_ptr.add(allocation_bit * $slab_size) })
                    } else {
                        None
                    }
                }) {
                    Some(allocation_ptr) if allocation_ptr_cell.set(allocation_ptr).is_err() => unreachable!(),
                    None if let Ok(address) = $self.lock_next() => {
                        slabs.push((
                            // SAFETY: `phys_mapped_address` is required to be valid for arbitrary offsets from within its range.
                            unsafe { $self.phys_mapped_address.as_mut_ptr::<u8>().add(address.as_usize()) },
                            0
                        ));
                    }

                    _ => unimplemented!()
                }
            }

            // SAFETY: This code is only reached once `allocation_ptr` is no longer `None`.
            Ok(slice_from_raw_parts_mut(unsafe { allocation_ptr_cell.take().unwrap_unchecked() }, $slab_size))
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
                    self.lock_next_many(layout.size() / 0x1000)
                })
                .map(|address| {
                    // SAFETY: Frame addresses are naturally aligned, and arbitrary memory is valid for `u8`, and `phys_mapped_address` is
                    //         required to be valid for arbitrary offsets from within its range.
                    let allocation_ptr = unsafe { self.phys_mapped_address.as_mut_ptr::<u8>().add(address.as_usize()) };
                    slice_from_raw_parts_mut(allocation_ptr, libkernel::align_up(layout.size(), 4096))
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
