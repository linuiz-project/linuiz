use crate::{interrupts::InterruptCell, mem::hhdm};
use bitvec::slice::BitSlice;
use core::{
    alloc::{AllocError, Allocator, Layout},
    num::{NonZeroU32, NonZeroUsize},
    ptr::NonNull,
    sync::atomic::AtomicUsize,
};
use libsys::{Address, Frame, page_mask, page_shift, page_size};
use spin::RwLock;

#[derive(Debug, Clone, Copy)]
pub struct InitError;

static PMM: spin::Once<PhysicalMemoryManager> = spin::Once::new();

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
}

pub type Result<T> = core::result::Result<T, Error>;
type FrameTable = RwLock<&'static mut BitSlice<AtomicUsize>>;

pub struct PhysicalMemoryManager {
    table: InterruptCell<FrameTable>,
    total_frames: usize,
}

// Safety: Type uses entirely atomic operations.
unsafe impl Send for PhysicalMemoryManager {}
// Safety: Type uses entirely atomic operations.
unsafe impl Sync for PhysicalMemoryManager {}

impl PhysicalMemoryManager {
    /// Initializes the static physical memory manager with the provided bootloader memory map request.
    pub fn init(memory_map_request: &limine::request::MemoryMapRequest) {
        debug_assert!(
            !PMM.is_completed(),
            "physical memory manager is already initialized"
        );

        PMM.call_once(|| {
            let memory_map = memory_map_request
                .get_response()
                .expect("no response to memory map request")
                .entries();

            let free_ranges = memory_map
                .iter()
                .filter(|&entry| entry.entry_type == limine::memory_map::EntryType::USABLE)
                .map(|entry| {
                    let region_start = usize::try_from(entry.base).unwrap();
                    let region_end = usize::try_from(entry.base + entry.length).unwrap();

                    region_start..region_end
                });

            let total_memory = memory_map.iter().map(|e| e.base + e.length).max().unwrap();
            let total_memory = usize::try_from(total_memory).unwrap();
            trace!("Total phyiscal memory: {}M", total_memory / 1_000_000);

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

            let select_region = free_ranges
                .filter(|region| (region.start & page_mask()) == 0)
                .find(|region| region.len() >= table_size_in_bytes)
                .map(|region| region.start..(region.start + table_size_in_bytes))
                .expect("bootloader provided no free regions large enough for frame table");

            assert_eq!(select_region.start & page_mask(), 0);
            assert_eq!(select_region.end & page_mask(), 0);

            trace!("Frame table region: {:X?}", select_region);

            // Safety: Memory map describes HHDM, so this pointer into it will be valid if the bootloader memory map is.s
            let table_start_ptr = unsafe { hhdm::get().ptr().add(select_region.start) };
            // Safety: Unless the memory map lied to us, this memory is valid for a `&[AtomicUsize; total_frames]`.
            let table = BitSlice::from_slice_mut(unsafe {
                core::slice::from_raw_parts_mut(
                    table_start_ptr.cast::<AtomicUsize>(),
                    table_slice_len,
                )
            });
            // Clear the table's bits, so we can populate it later.
            table.fill(false);

            // Fill the extant bits, as the table may have more bits than there are frames.
            table[total_frames..].fill(true);

            // Ensure the table's frames are reserved.
            let table_start_index = select_region.start / page_size();
            let table_end_index = select_region.end / page_size();
            table[table_start_index..table_end_index].fill(true);

            Self {
                table: InterruptCell::new(spin::RwLock::new(table)),
                total_frames,
            }
        });
    }

    fn get_static() -> &'static Self {
        PMM.get()
            .expect("physical memory manager has not been initialized")
    }

    /// Passes the static physical memory manager's frame table to `with_fn`, returning the result.
    fn with_table<T>(with_fn: impl FnOnce(&FrameTable) -> Result<T>) -> Result<T> {
        Self::get_static().table.with(with_fn)
    }

    pub fn total_frames() -> usize {
        Self::get_static().total_frames
    }

    pub fn total_memory() -> usize {
        Self::total_frames() * libsys::page_size()
    }

    pub fn next_frame() -> Result<Address<Frame>> {
        Self::with_table(|table| {
            let mut table = table.write();
            let index = table.first_zero().ok_or(Error::NoneFree)?;
            table.set(index, true);

            Ok(Address::new(index << page_shift().get()).unwrap())
        })
    }

    pub fn next_frames(
        count: NonZeroUsize,
        align_bits: Option<NonZeroU32>,
    ) -> Result<Address<Frame>> {
        Self::with_table(|table| {
            let mut table = table.write();

            let align_bits = align_bits.unwrap_or(NonZeroU32::MIN).get();
            let align_index_skip = u32::max(1, align_bits >> page_shift().get());

            let free_frames_index = table
                .windows(count.get())
                .enumerate()
                .step_by(align_index_skip.try_into().unwrap())
                .find_map(|(index, window)| window.not_any().then_some(index))
                .ok_or(Error::NoneFree)?;

            // It's a bit uglier to find the index of the window, then effectively reacreate it. However, `.windows()`
            // does not return a mutable bitslice, so this is how it must be done.
            let free_frames = table
                .get_mut(free_frames_index..(free_frames_index + count.get()))
                .unwrap();
            free_frames.fill(true);

            Ok(Address::new(free_frames_index << page_shift().get()).unwrap())
        })
    }

    pub fn lock_frame(address: Address<Frame>) -> Result<()> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index < Self::total_frames() {
                // if the frame is free...
                if table[index] {
                    table.set_aliased(index, true);

                    Ok(())
                } else {
                    Err(Error::NotFree)
                }
            } else {
                Err(Error::OutOfBounds)
            }
        })
    }

    pub fn free_frame(address: Address<Frame>) -> Result<()> {
        Self::with_table(|table| {
            let table = table.read();
            let index = address.index();

            // The table may have more bits than there are frames due to the
            // padding effect of using a `usize` as the underlying data type.
            if index < Self::total_frames() {
                // if the frame is locked...
                if !table[index] {
                    table.set_aliased(index, false);

                    Ok(())
                } else {
                    Err(Error::NotLocked)
                }
            } else {
                Err(Error::OutOfBounds)
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
                Self::next_frames(NonZeroUsize::new(frame_count).unwrap(), Some(page_shift()))
            }
            core::cmp::Ordering::Equal => Self::next_frame(),
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
            Self::free_frame(address).ok();
        } else {
            let frame_count = libsys::align_up_div(layout.size(), page_shift());
            let frames_start = address.index();
            let frames_end = frames_start + frame_count;

            (frames_start..frames_end)
                .map(Address::from_index)
                .map(Option::unwrap)
                .try_for_each(Self::free_frame)
                .expect("failed while freeing frames");
        }
    }
}
