use crate::Address;

#[repr(u32)]
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryType {
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
    PERSISTENT,
    UNACCEPTED,
    KERNEL = 0xFFFFFF00,
}

bitflags::bitflags! {
    pub struct MemoryAttributes: u64 {
        const UNCACHEABLE = 0x1;
        const WRITE_COMBINE = 0x2;
        const WRITE_THROUGH = 0x4;
        const WRITE_BACK = 0x8;
        const UNCACHABLE_EXPORTED = 0x10;
        const WRITE_PROTECT = 0x1000;
        const READ_PROTECT = 0x2000;
        const EXEC_PROTECT = 0x4000;
        const NON_VOLATILE = 0x8000;
        const MORE_RELIABLE = 0x10000;
        const READ_ONLY = 0x20000;
        const RUNTIME = 0x8000_0000_0000_0000;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct MemoryDescriptor {
    pub ty: MemoryType,
    ty_padding: u32,
    pub phys_start: Address<crate::Physical>,
    pub virt_start: Address<crate::Virtual>,
    pub page_count: u64,
    pub att: MemoryAttributes,
}

impl MemoryDescriptor {
    pub fn range(&self) -> core::ops::Range<u64> {
        let addr_u64 = self.phys_start.as_usize() as u64;
        addr_u64..(addr_u64 + (self.page_count * 0x1000))
    }

    pub fn frame_range(&self) -> core::ops::RangeInclusive<usize> {
        let start_index = self.phys_start.frame_index();
        start_index..=(start_index + (self.page_count as usize))
    }

    pub fn is_stack_descriptor(&self) -> bool {
        self.range()
            .contains(&(crate::registers::stack::RSP::read() as u64))
    }

    pub fn should_reserve(&self) -> bool {
        !matches!(
            self.ty,
                  MemoryType::BOOT_SERVICES_CODE
                | MemoryType::BOOT_SERVICES_DATA
                | MemoryType::LOADER_CODE
                | MemoryType::LOADER_DATA
                | MemoryType::CONVENTIONAL
                // TODO possibly specify this memory separately?
                | MemoryType::PERSISTENT
        )
        // If this is a stack descriptor, it should be reserved.
        //
        // I'm not sure if we can count on BIOS always using the same descriptor type
        //  for the stack descriptor, so this is a more robust way to handle that possibility.
        || self.is_stack_descriptor()
    }
}
