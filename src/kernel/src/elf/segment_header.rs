use libkernel::{Address, Physical, Virtual};

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct SegmentFlags : u32 {
        const EXECUTABLE    = 1 << 0;
        const WRITABLE      = 1 << 1;
        const READABLE      = 1 << 1;
    }
}

#[repr(u32)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(dead_code, non_camel_case_types)]
pub enum SegmentType {
    NULL = 0x0,
    LOAD = 0x1,
    DYNAMIC = 0x2,
    INTERP = 0x3,
    NOTE = 0x4,
    SHLIB = 0x5,
    PHDR = 0x6,
    TLS = 0x7,
    LOOS = 0x60000000,
    HIOS = 0x6FFFFFFF,
    LOPROC = 0x70000000,
    HIPROC = 0x7FFFFFFF,
}

#[repr(C)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct SegmentHeader {
    pub ty: SegmentType,
    pub flags: SegmentFlags,
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
            .field("Type", &self.ty)
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
