mod mapper;
pub use mapper::*;

use crate::{
    interrupts::InterruptCell,
    memory::{PageAttributes, PhysicalAllocator},
};
use alloc::collections::BTreeMap;
use core::{
    alloc::{Allocator, Layout},
    num::NonZeroUsize,
    ops::ControlFlow,
    ptr::NonNull,
};
use libsys::{page_size, Address, Pow2Usize};
use spin::{Lazy, Mutex, RwLock};
use try_alloc::vec::TryVec;
use uuid::Uuid;

use super::Virtual;

static ADDRESS_SPACES: InterruptCell<
    Lazy<RwLock<BTreeMap<Uuid, Mutex<AddressSpace<PhysicalAllocator>>, PhysicalAllocator>>>,
> = InterruptCell::new(Lazy::new(|| RwLock::new(BTreeMap::new_in(&*super::PMM))));

pub fn register(uuid: Uuid, size: NonZeroUsize) -> Result<(), Error> {
    let address_space = unsafe { AddressSpace::new_in(size, &*super::PMM).map_err(|_| Error) }?;

    ADDRESS_SPACES.with(|address_spaces| {
        let mut guard = address_spaces.write();
        guard.try_insert(uuid, Mutex::new(address_space)).map(|_| ()).map_err(|_| Error)
    })
}

pub fn with<T>(uuid: &Uuid, func: impl FnOnce(&mut AddressSpace<PhysicalAllocator>) -> T) -> Option<T> {
    ADDRESS_SPACES.with(|address_spaces| {
        let address_spaces = address_spaces.read();
        address_spaces.get(uuid).map(|address_space| {
            let mut address_space = address_space.lock();
            func(&mut *address_space)
        })
    })
}

bitflags::bitflags! {
    pub struct MmapFlags : u16 {
        const READ = 0b1;
        const READ_WRITE = 0b11;
        const READ_EXECUTE = 0b111;
        const NOT_DEMAND = 0b1000;
    }
}

impl From<MmapFlags> for PageAttributes {
    fn from(flags: MmapFlags) -> Self {
        let mut attributes = PageAttributes::empty();

        if !flags.contains(MmapFlags::NOT_DEMAND) {
            attributes.insert(PageAttributes::DEMAND);
        }

        if flags.contains(MmapFlags::READ_EXECUTE) {
            attributes.insert(PageAttributes::RX);
        } else if flags.contains(MmapFlags::READ_WRITE) {
            attributes.insert(PageAttributes::RW);
        } else if flags.contains(MmapFlags::READ) {
            attributes.insert(PageAttributes::RO);
        }

        attributes
    }
}

impl From<PageAttributes> for MmapFlags {
    fn from(attributes: PageAttributes) -> Self {
        let mut flags = MmapFlags::empty();

        if !attributes.contains(PageAttributes::DEMAND) {
            flags.insert(MmapFlags::NOT_DEMAND)
        }

        if attributes.contains(PageAttributes::RX) {
            flags.insert(MmapFlags::READ_EXECUTE);
        } else if attributes.contains(PageAttributes::RW) {
            flags.insert(MmapFlags::READ_WRITE);
        } else if attributes.contains(PageAttributes::RO) {
            flags.insert(MmapFlags::READ);
        }

        flags
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Error;

#[derive(Debug, Clone, Copy)]
struct Region {
    len: usize,
    free: bool,
}

pub struct AddressSpace<A: Allocator + Clone> {
    regions: TryVec<Region, A>,
    allocator: A,
    mapper: Mapper,
}

impl<A: Allocator + Clone> AddressSpace<A> {
    pub unsafe fn new_in(size: NonZeroUsize, allocator: A) -> Result<Self, Error> {
        let mut vec = TryVec::new_in(allocator.clone());
        vec.push(Region { len: size.get(), free: true }).map_err(|_| Error)?;

        Ok(Self { regions: vec, allocator, mapper: Mapper::new().ok_or(Error)? })
    }

    // TODO better error type for this function
    pub fn mmap(
        &mut self,
        // TODO
        // address: Option<Address<Virtual>>,
        layout: Layout,
        flags: MmapFlags,
    ) -> Result<NonNull<[u8]>, Error> {
        let layout = Layout::from_size_align(layout.size(), core::cmp::max(layout.align(), page_size().get()))
            .map_err(|_| Error)?;
        // Safety: `Layout` does not allow `0` for alignments.
        let layout_align = Pow2Usize::new(layout.align()).unwrap();

        let search = self.regions.iter().try_fold((0usize, 0usize), |(index, address), region| {
            let aligned_address = libsys::align_up(address, layout_align);
            let aligned_padding = aligned_address - address;
            let aligned_len = region.len.saturating_sub(aligned_padding);

            if aligned_len < layout.size() {
                ControlFlow::Continue((index + 1, address + region.len))
            } else {
                ControlFlow::Break((index, address))
            }
        });

        match search.break_value() {
            None => Err(Error),

            Some((mut index, address)) => {
                let aligned_address = libsys::align_up(address, layout_align);
                let aligned_padding = aligned_address - address;

                if aligned_padding > 0 {
                    if let Some(region) = self.regions.get_mut(index) && region.free {
                        region.len += aligned_padding;
                    } else {
                        self.regions.insert(index, Region { len: aligned_padding, free: true }).map_err(|_| Error)?;
                        index += 1;
                    }
                }

                let remaining_len = {
                    let region = self.regions.get_mut(index).ok_or(Error)?;
                    region.len -= aligned_padding;
                    region.free = false;

                    let len = region.len;
                    region.len = layout.size();
                    len - region.len
                };

                if remaining_len > 0 {
                    if let Some(region) = self.regions.get_mut(index.saturating_add(1)) && region.free {
                        region.len += remaining_len;
                    } else {
                        self.regions.insert(index + 1, Region { len: remaining_len, free: true }).map_err(|_| Error)?;
                    }
                }

                // Set up paging attributes based on provided mmap flags.
                let mut attributes = {
                    if flags.contains(MmapFlags::READ_EXECUTE) {
                        PageAttributes::RX
                    } else if flags.contains(MmapFlags::READ_WRITE) {
                        PageAttributes::RW
                    } else if flags.contains(MmapFlags::READ) {
                        PageAttributes::RO
                    } else {
                        PageAttributes::empty()
                    }
                };
                // Demand paging is the default, but optionally the user can specify front-loading the physical page allocations.
                if !flags.contains(MmapFlags::NOT_DEMAND) {
                    attributes.insert(PageAttributes::DEMAND);
                }
                // Finally, map all of the allocated pages in the virtual address space.
                for page_base in (aligned_address..(aligned_address + layout.size())).step_by(page_size().get()) {
                    let page = Address::new(page_base).ok_or(Error)?;
                    self.mapper.auto_map(page, attributes).map_err(|_| Error)?;
                }

                NonNull::new(aligned_address as *mut u8)
                    .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
                    .ok_or(Error)
            }
        }
    }

    pub fn demand_map(&mut self, address: Address<Virtual>) -> Result<(), Error> {
        let page = Address::new_truncate(address.get());
        // TODO we need to return the page size from `get_page_attributes` or something, so when we clear from a page fault, it clears huge pages too.
        match self.mapper.get_page_attributes(page) {
            Some(mut attributes) if attributes.contains(PageAttributes::DEMAND) => {
                self.mapper
                    .auto_map(page, {
                        // remove demand bit ...
                        attributes.remove(PageAttributes::DEMAND);
                        // ... insert present bit ...
                        attributes.insert(PageAttributes::PRESENT);
                        // ... return attributes
                        attributes
                    })
                    .unwrap();

                // Safety: We know the page was just mapped, and contains no relevant memory.
                unsafe { core::ptr::write_bytes(page.as_ptr(), 0, page_size().get()) };

                Ok(())
            }

            _ => Err(Error),
        }
    }

    pub fn is_mmapped(&self, address: Address<Virtual>) -> bool {
        self.mapper.is_mapped(Address::new_truncate(address.get()), None)
    }
}
