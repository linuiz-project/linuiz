use crate::memory::{Frame, FrameIterator};
use x86_64::{PhysAddr, VirtAddr};

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UEFIMemoryType {
    RESERVED,
    LOADER_CODE,
    LOADER_DATA,
    BOOT_SERVICES_CODE,
    BOOT_SERVICES_DATA,
    RUNTIME_SERVICES_CODE,
    RUNTIME_SERVICES_DATA,
    CONVENTIONAL,
    UNUSABLE,
    ACPI_RECLAIM,
    ACPI_NON_VOLATILE,
    MMIO,
    MMIO_PORT_SPACE,
    PAL_CODE,
    PERSISTENT_MEMORY,
    KERNEL_CODE = 0xFFFFFF00,
    KERNEL_DATA = 0xFFFFFF01,
}

bitflags::bitflags! {
    pub struct UEFIMemoryAttribute: u64 {
        const UNCACHEABLE = 0x1;
        const WRITE_COMBINE = 0x2;
        const WRITE_THROUGH = 0x4;
        const WRITE_BACK = 0x8;
        const UNCACHABLE_EXPORTED = 0x10;
        const WRITE_PROTECT = 0x1000;
        const READ_PROTECT = 0x2000;
        const EXECUTE_PROTECT = 0x4000;
        const NON_VOLATILE = 0x8000;
        const MORE_RELIABLE = 0x10000;
        const READ_ONLY = 0x20000;
        const RUNTIME = 0x8000_0000_0000_0000;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct UEFIMemoryDescriptor {
    pub ty: UEFIMemoryType,
    padding: u32,
    pub phys_start: PhysAddr,
    pub virt_start: VirtAddr,
    pub page_count: u64,
    pub att: UEFIMemoryAttribute,
}

impl UEFIMemoryDescriptor {
    pub fn range(&self) -> core::ops::Range<u64> {
        let addr_u64 = self.phys_start.as_u64();
        addr_u64..(addr_u64 + (self.page_count * 0x1000))
    }

    pub fn frame_iter(&self) -> FrameIterator {
        Frame::range_count(Frame::from_addr(self.phys_start), self.page_count as usize)
    }

    pub fn is_stack_descriptor(&self) -> bool {
        self.range()
            .contains(&crate::registers::stack::RSP::read().as_u64())
    }

    pub fn should_reserve(&self) -> bool {
        match self.ty {
            UEFIMemoryType::BOOT_SERVICES_CODE
            | UEFIMemoryType::BOOT_SERVICES_DATA
            | UEFIMemoryType::LOADER_CODE
            | UEFIMemoryType::LOADER_DATA
            | UEFIMemoryType::CONVENTIONAL => {
                // If this is a stack descriptor, it should be reserved.
                //
                // I'm not sure if we can count on BIOS always using the same descriptor type
                //  for the stack descriptor, so this is a more robust way to handle that possibility.
                self.is_stack_descriptor()
            }
            _ => true,
        }
    }
}
