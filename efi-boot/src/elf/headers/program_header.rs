#[repr(u32)]
#[allow(unused_imports, non_camel_case_types)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ProgramHeaderType {
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
pub struct ProgramHeader {
    ph_type: ProgramHeaderType,
    flags: u32,
    offset: usize,
    vaddr: usize,
    paddr: usize,
    disk_size: usize,
    mem_size: usize,
    seg_flags: u32,
    alignment: usize,
}

impl ProgramHeader {
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        // verify length of passed slice
        if bytes.len() < core::mem::size_of::<ProgramHeader>() {
            None
        } else {
            unsafe {
                let header_ptr = bytes.as_ptr() as *const ProgramHeader;
                // this version of the header relies on the buffer data, which is unsafe
                let temp_header = *header_ptr;
                // so we return a clone
                Some(temp_header.clone())
            }
        }
    }

    pub fn ph_type(&self) -> ProgramHeaderType {
        self.ph_type
    }

    /// offset of the segment in the file image
    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn virtual_address(&self) -> usize {
        self.vaddr
    }

    pub fn physical_address(&self) -> usize {
        self.paddr
    }

    pub fn disk_size(&self) -> usize {
        self.disk_size
    }

    pub fn memory_size(&self) -> usize {
        self.mem_size
    }
}

impl core::fmt::Debug for ProgramHeader {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> Result<(), core::fmt::Error> {
        formatter
            .debug_struct("Program Header")
            .field("Type", &self.ph_type)
            .field("Flags", &self.flags)
            .field("Offset", &self.offset)
            .field("VAaddr", &self.vaddr)
            .field("PAddr", &self.paddr)
            .field("Disk Size", &self.disk_size)
            .field("Mem Size", &self.mem_size)
            .field("Alignment", &self.alignment)
            .finish()
    }
}
