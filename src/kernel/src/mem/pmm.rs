use crate::{interrupts::InterruptCell, mem::hhdm};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::{NonZeroU32, NonZeroUsize},
    ops::Range,
    ptr::NonNull,
    sync::atomic::AtomicUsize,
};
use libsys::{Address, Frame};
use libsys::{page_mask, page_shift, page_size};
use spin::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct InitError;

static PMM: spin::Once<PhysicalMemoryManager> = spin::Once::new();

pub fn init(memory_map: &[&limine::memory_map::Entry]) {
    PMM.call_once(|| {
        let free_regions = memory_map
            .iter()
            .filter(|&entry| entry.entry_type == limine::memory_map::EntryType::USABLE)
            .map(|entry| {
                let region_start = usize::try_from(entry.base).unwrap();
                let region_end = usize::try_from(entry.base + entry.length).unwrap();

                region_start..region_end
            });

        let total_memory = memory_map
            .iter()
            .map(|e| e.base + e.length)
            .max()
            .unwrap()
            .try_into()
            .unwrap();
        trace!("Total phyiscal memory: {total_memory:#X}");

        PhysicalMemoryManager::new(free_regions, total_memory).expect("failed creating pmm")
    });
}

pub fn get() -> &'static PhysicalMemoryManager {
    PMM.get()
        .expect("physical memory manager has not been initialized")
}

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

pub struct PhysicalMemoryManager {
    table: InterruptCell<RwLock<&'static mut BitSlice<AtomicUsize>>>,
}

// Safety: Type uses entirely atomic operations.
unsafe impl Send for PhysicalMemoryManager {}
// Safety: Type uses entirely atomic operations.
unsafe impl Sync for PhysicalMemoryManager {}

impl PhysicalMemoryManager {
    pub fn new(
        free_regions: impl Iterator<Item = Range<usize>>,
        total_memory: usize,
    ) -> Option<Self> {
        let total_frames = total_memory / page_size();
        let table_slice_len = libsys::align_up_div(
            total_frames,
            NonZeroU32::new(usize::BITS.trailing_zeros()).unwrap(),
        );
        let table_size_in_frames = libsys::align_up_div(
            table_slice_len * core::mem::size_of::<usize>(),
            page_shift(),
        );
        let table_size_in_bytes = table_size_in_frames * page_size();

        let select_region = free_regions
            .filter(|region| (region.start & page_mask()) == 0)
            .find(|region| region.len() >= table_size_in_bytes)
            .map(|region| region.start..(region.start + table_size_in_bytes))?;

        assert_eq!(select_region.start & page_mask(), 0);
        assert_eq!(select_region.end & page_mask(), 0);

        trace!("Selecting region for ledger: {:X?}", select_region);

        // Safety: Memory map describes HHDM, so this pointer into it will be valid if the bootloader memory map is.s
        let ledger_start_ptr = unsafe { hhdm::get().ptr().add(select_region.start) };
        // Safety: Unless the memory map lied to us, this memory is valid for a `&[AtomicUsize; total_frames]`.
        let ledger = BitSlice::from_slice_mut(unsafe {
            core::slice::from_raw_parts_mut(ledger_start_ptr.cast::<AtomicUsize>(), table_slice_len)
        });
        ledger.fill(false);

        // Fill the extant bits, as the physical memory bitslice may not be exactly divisible by `usize::BITS`.
        ledger[total_frames..(table_slice_len * (usize::BITS as usize))].fill(true);

        // Ensure the table pages are reserved.
        let ledger_start_index = select_region.start / page_size();
        let ledger_end_index = select_region.end / page_size();
        ledger[ledger_start_index..ledger_end_index].fill(true);

        Some(Self {
            table: InterruptCell::new(spin::RwLock::new(ledger)),
        })
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

    pub fn next_frames(
        &self,
        count: NonZeroUsize,
        align_bits: Option<NonZeroU32>,
    ) -> Result<Address<Frame>> {
        let align_bits = align_bits.unwrap_or(NonZeroU32::MIN).get();
        let align_index_skip = u32::max(1, align_bits >> page_shift().get());
        self.table.with(|table| {
            let mut table = table.write();
            let index = table
                .windows(count.get())
                .enumerate()
                .step_by(align_index_skip.try_into().unwrap())
                // TODO simplify this to return the window directly.
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

// Safety: PMM utilizes interior mutability & Correct:tm: logic.
unsafe impl Allocator for PhysicalMemoryManager {
    fn allocate(&self, layout: Layout) -> core::result::Result<NonNull<[u8]>, AllocError> {
        assert!(layout.align() <= page_size());

        let frame_count = libsys::align_up_div(layout.size(), page_shift());
        let frame = match frame_count.cmp(&1usize) {
            core::cmp::Ordering::Greater => {
                self.next_frames(NonZeroUsize::new(frame_count).unwrap(), Some(page_shift()))
            }
            core::cmp::Ordering::Equal => self.next_frame(),
            core::cmp::Ordering::Less => unreachable!(),
        }
        .map_err(|_| AllocError)?;
        let address = hhdm::get().offset(frame).ok_or(AllocError)?;

        Ok(NonNull::slice_from_raw_parts(
            NonNull::new(address.as_ptr()).unwrap(),
            frame_count * page_size(),
        ))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        assert!(layout.align() <= page_size());

        let offset = ptr.addr().get() - hhdm::get().virt().get();
        let address = Address::new(offset).unwrap();

        if layout.size() <= page_size() {
            self.free_frame(address).ok();
        } else {
            let frame_count = libsys::align_up_div(layout.size(), page_shift());
            for index_offset in 0..frame_count {
                self.free_frame(Address::from_index(address.index() + index_offset).unwrap())
                    .ok();
            }
        }
    }
}
