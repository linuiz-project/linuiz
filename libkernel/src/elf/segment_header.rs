use crate::{
    addr_ty::{Physical, Virtual},
    Address,
};

#[repr(u32)]
#[allow(dead_code, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SegmentType {
    PT_NULL = 0x0,
    PT_LOAD = 0x1,
    PT_DYNAMIC = 0x2,
    PT_INTERP = 0x3,
    PT_NOTE = 0x4,
    PT_SHLIB = 0x5,
    PT_PHDR = 0x6,
    PT_TLS = 0x7,
    PT_LOOS = 0x60000000,
    PT_HIOS = 0x6FFFFFFF,
    PT_LOPROC = 0x70000000,
    PT_HIPROC = 0x7FFFFFFF,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SegmentHeader {
    pub ph_type: SegmentType,
    pub flags: u32,
    pub offset: usize,
    pub virt_addr: Address<Virtual>,
    pub phys_addr: Address<Physical>,
    pub disk_size: usize,
    pub mem_size: usize,
    pub align: usize,
}

impl core::fmt::Debug for SegmentHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("Segment Header")
            .field("Type", &self.ph_type)
            .field("Flags", &self.flags)
            .field("Offset", &self.offset)
            .field("Virtual Address", &self.virt_addr)
            .field("Physical Address", &self.phys_addr)
            .field("Disk Size", &self.disk_size)
            .field("Memory Size", &self.mem_size)
            .field("Alignment", &self.align)
            .finish()
    }
}
