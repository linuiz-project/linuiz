use libcommon::{Address, Physical, Virtual};

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
    GnuStack,
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
            0x6474E551 => Self::GnuStack,
            0x60000000..0x6FFFFFFF => Self::OsSpecific(value),
            0x70000000..0x7FFFFFFF => Self::ProcessorSpecific(value),
            _ => unimplemented!(),
        }
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy)]
pub struct Header {
    ty: u32,
    flags: u32,
    offset: u64,
    virt_addr: u64,
    phys_addr: u64,
    disk_size: u64,
    mem_size: u64,
    align: u64,
}

// ### Safety: Type is composed of simple primitive numerics.
unsafe impl bytemuck::AnyBitPattern for Header {}
// ### Safety: Type is composed of simple primitive numerics.
unsafe impl bytemuck::Zeroable for Header {}

impl Header {
    #[inline]
    pub const fn get_type(&self) -> Type {
        Type::from_u32(self.ty)
    }

    #[inline]
    pub const fn get_flags(&self) -> Flags {
        Flags::from_bits_truncate(self.flags)
    }

    #[inline]
    pub const fn get_file_offset(&self) -> usize {
        self.offset as usize
    }

    #[inline]
    pub fn get_virtual_address(&self) -> Option<Address<Virtual>> {
        Address::<Virtual>::new(self.virt_addr)
    }

    #[inline]
    pub fn get_physical_address(&self) -> Option<Address<Physical>> {
        Address::<Physical>::new(self.phys_addr)
    }

    #[inline]
    pub const fn get_disk_size(&self) -> usize {
        self.disk_size as usize
    }

    #[inline]
    pub fn get_memory_layout(&self) -> Result<core::alloc::Layout, core::alloc::LayoutError> {
        core::alloc::Layout::from_size_align(self.mem_size as usize, self.align as usize)
    }
}

pub struct Segment<'a> {
    header: &'a Header,
    data: &'a [u8],
}

impl core::ops::Deref for Segment<'_> {
    type Target = Header;

    fn deref(&self) -> &Self::Target {
        self.header
    }
}

impl<'a> Segment<'a> {
    #[inline]
    pub const fn new(header: &'a Header, data: &'a [u8]) -> Self {
        Self { header, data }
    }

    #[inline]
    pub const fn data(&self) -> &[u8] {
        self.data
    }
}

impl core::fmt::Debug for Segment<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
        formatter
            .debug_struct("Segment")
            .field("Type", &self.get_type())
            .field("Flags", &self.get_flags())
            .field("File Offset", &self.get_file_offset())
            .field("Disk Size", &self.get_disk_size())
            .field("Virtual Address", &self.get_virtual_address())
            .field("Physical Address", &self.get_physical_address())
            .field("Memory Layout", &self.get_memory_layout())
            .finish()
    }
}
