use libkernel::{Address, Physical, Virtual};

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct Flags : u32 {
        const EXECUTABLE    = 1 << 0;
        const WRITABLE      = 1 << 1;
        const READABLE      = 1 << 1;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum Type {
    Unused,
    Loadable,
    Dynamic,
    Interpreter,
    Note,
    ProgramHeaderTable,
    ThreadLocalStorage,
    OsSpecific(u32),
    ProcessorSpecific(u32),
}

impl Type {
    const fn from_u32(value: u32) -> Self {
        match value {
            0x0 => Self::Unused,
            0x1 => Self::Loadable,
            0x2 => Self::Dynamic,
            0x3 => Self::Interpreter,
            0x4 => Self::Note,
            0x6 => Self::ProgramHeaderTable,
            0x7 => Self::ThreadLocalStorage,
            0x60000000..0x6FFFFFFF => Self::OsSpecific(value),
            0x70000000..0x7FFFFFFF => Self::ProcessorSpecific(value),
            _ => unreachable!(),
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, PartialEq, Eq)]
pub struct Header {
    ty: u32,
    flags: u32,
    offset: u64,
    virt_addr: Address<Virtual>,
    phys_addr: Address<Physical>,
    disk_size: u64,
    mem_size: u64,
    align: u64,
}

impl Header {
    pub const fn get_type(&self) -> Type {
        Type::from_u32(self.ty)
    }

    pub const fn get_flags(&self) -> Flags {
        Flags::from_bits_truncate(self.flags)
    }

    pub const fn get_file_address(&self) -> u64 {
        self.offset
    }
}

impl core::fmt::Debug for Header {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("Segment Header")
            // .field("Type", &self.ty)
            // .field("Flags", &self.flags)
            // .field("Offset", &self.offset)
            // .field("Virtual Address", &self.virt_addr)
            // .field("Physical Address", &self.phys_addr)
            // .field("Disk Size", &self.disk_size)
            // .field("Memory Size", &self.mem_size)
            // .field("Alignment", &self.align)
            .finish()
    }
}
