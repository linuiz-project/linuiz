use crate::{interrupts::InterruptCell, mem::Hhdm};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::{NonZeroU32, NonZeroUsize},
    ops::Range,
    ptr::NonNull,
    sync::atomic::AtomicUsize,
};
use libsys::{page_mask, page_shift, page_size};
use libsys::{Address, Frame};
use spin::RwLock;

pub type PhysicalAllocator = &'static PhysicalMemoryManager<'static>;

pub static PMM: spin::Lazy<PhysicalMemoryManager> = spin::Lazy::new(|| {
    let memory_map = crate::boot::get_memory_map().unwrap();

    let free_regions = memory_map.iter().map(|region| {
        let region = region.range();
        let region_start = usize::try_from(region.start).unwrap();
        let region_end = usize::try_from(region.end).unwrap();

        region_start..region_end
    });
    let total_memory = usize::try_from(memory_map.iter().last().unwrap().range().end).unwrap();

    PhysicalMemoryManager { allocator: FrameAllocator::new(free_regions, total_memory).unwrap() }
});

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

    TypeMismatch,

    Unknown,
}

pub type Result<T> = core::result::Result<T, Error>;

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

struct RegionDescriptor {
    ty: FrameType,
    region: Range<usize>,
}

pub struct PhysicalMemoryManager<'a> {
    // TODO map: Vec<RegionDescriptor, &'a FrameAllocator<'a>>,
    allocator: FrameAllocator<'a>,
}

impl<'a> core::ops::Deref for PhysicalMemoryManager<'a> {
    type Target = FrameAllocator<'a>;

    fn deref(&self) -> &Self::Target {
        &self.allocator
    }
}

unsafe impl Allocator for &PhysicalMemoryManager<'_> {
    fn allocate(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        let count = NonZeroUsize::new(libsys::align_up_div(layout.size(), page_shift())).ok_or(AllocError)?;
        let frame = {
            if layout.size() <= page_size() {
                self.next_frame()
            } else {
                self.next_frames(count, Some(page_shift()))
            }
        }
        .map_err(|_| AllocError)?;
        let address = Hhdm::offset(frame).ok_or(AllocError)?;

        Ok(NonNull::slice_from_raw_parts(NonNull::new(address.as_ptr()).unwrap(), page_size()))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.align() <= page_size());

        let offset = ptr.as_ptr().sub_ptr(Hhdm::address().as_ptr());
        let address = Address::new(offset).unwrap();

        if layout.size() <= page_size() {
            self.free_frame(address).ok();
        } else {
            let frame_count = libsys::align_up_div(layout.size(), page_shift());
            for index_offset in 0..frame_count {
                self.free_frame(Address::from_index(address.index() + index_offset).unwrap()).ok();
            }
        }
    }
}

pub struct FrameAllocator<'a> {
    table: InterruptCell<RwLock<&'a mut BitSlice<AtomicUsize>>>,
}

// Safety: Type uses entirely atomic operations.
unsafe impl Send for FrameAllocator<'_> {}
// Safety: Type uses entirely atomic operations.
unsafe impl Sync for FrameAllocator<'_> {}

impl FrameAllocator<'_> {
    pub fn new(free_regions: impl Iterator<Item = Range<usize>> + Clone, total_memory: usize) -> Option<Self> {
        let total_frames = total_memory / page_size();
        let table_slice_len = total_frames / (usize::BITS as usize);
        let table_size_in_bytes = table_slice_len * core::mem::size_of::<usize>();
        let table_size_in_frames = libsys::align_up_div(table_size_in_bytes, page_shift());

        let select_entry = free_regions
            .clone()
            .filter(|region| (region.start & page_mask()) == 0)
            .find(|region| region.len() >= table_size_in_bytes)?;
        let select_entry_frame_index = select_entry.start / page_size();

        // Safety: Unless the memory map lied to us, this memory is valid for a `&[FrameData; total_frames]`.
        let table = unsafe { core::slice::from_raw_parts_mut(select_entry.start as *mut AtomicUsize, table_slice_len) };
        let table = BitSlice::from_slice_mut(table);

        // Ensure the table pages are reserved.
        (&mut table[select_entry_frame_index..(select_entry_frame_index + table_size_in_frames)]).fill(true);

        Some(Self { table: InterruptCell::new(spin::RwLock::new(table)) })
    }

    #[inline]
    pub fn total_memory(&self) -> usize {
        self.table.with(|table| {
            let table = table.read();
            table.len() * libsys::page_size()
        })
    }

    pub fn next_frame(&self) -> Result<Address<Frame>> {
        self.table.with(|table| {
            let mut table = table.write();
            let index = table.first_zero().ok_or(Error::NoneFree)?;
            table.set(index, true);

            Ok(Address::new(index << page_shift().get()).unwrap())
        })
    }

    pub fn next_frames(&self, count: NonZeroUsize, align_bits: Option<NonZeroU32>) -> Result<Address<Frame>> {
        let align_bits = align_bits.unwrap_or(NonZeroU32::MIN).get();
        let align_index_skip = u32::max(1, align_bits >> page_shift().get());
        self.table.with(|table| {
            let mut table = table.write();
            let index = table
                .windows(count.get())
                .enumerate()
                .step_by(align_index_skip.try_into().unwrap())
                .find_map(|(index, window)| window.not_any().then_some(index))
                .ok_or(Error::NoneFree)?;
            let window = table.get_mut(index..(index + count.get())).unwrap();
            window.fill(true);

            Ok(Address::new(index << page_shift().get()).unwrap())
        })
    }

    pub fn lock_frame(&self, address: Address<Frame>) -> Result<()> {
        self.table.with(|table| {
            let table = table.read();
            let index = address.index();

            if index >= table.len() {
                Err(Error::OutOfBounds)
            } else {
                table.set_aliased(index, true);

                Ok(())
            }
        })
    }

    pub fn free_frame(&self, address: Address<Frame>) -> Result<()> {
        self.table.with(|table| {
            let table = table.read();
            let index = address.index();

            if index >= table.len() {
                Err(Error::OutOfBounds)
            } else {
                table.set_aliased(index, false);

                Ok(())
            }
        })
    }
}
