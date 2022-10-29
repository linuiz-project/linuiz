/// A symbol's binding determines the linkage visibility and behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bind {
    Local,
    Global,
    Weak,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Bind {
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Local,
            1 => Self::Global,
            2 => Self::Weak,
            10..13 => Self::OsSpecific(value),
            13..16 => Self::ProcessorSpecific(value),
            _ => unimplemented!(),
        }
    }
}

/// A symbol's type provides a general classification for the associated entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    None,
    Object,
    Function,
    Section,
    File,
    Common,
    Tls,
    OsSpecific(u8),
    ProcessorSpecific(u8),
}

impl Type {
    const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::None,
            1 => Self::Object,
            2 => Self::Function,
            3 => Self::Section,
            4 => Self::File,
            5 => Self::Common,
            6 => Self::Tls,
            10..13 => Self::OsSpecific(value),
            13..16 => Self::ProcessorSpecific(value),
            _ => unimplemented!(),
        }
    }
}

/// Visibility defines how that symbol can be accessed once the symbol has become part of an executable or shared object.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Default,
    Internal,
    Hidden,
    Protected,
    Exported,
    Singleton,
    Eliminate,
}

impl Visibility {
    pub const fn from_u8(value: u8) -> Self {
        match value {
            0 => Self::Default,
            1 => Self::Internal,
            2 => Self::Hidden,
            3 => Self::Protected,
            4 => Self::Exported,
            5 => Self::Singleton,
            6 => Self::Eliminate,
            _ => unimplemented!(),
        }
    }
}

#[repr(C, packed)]
#[derive(Default, Clone, Copy)]
pub struct Symbol {
    name_offset: u32,
    info: u8,
    visibility: u8,
    section_header_index: u16,
    value: u64,
    size: u64,
}

// SAFETY: Type is composed of simple primitive numerics.
unsafe impl bytemuck::Zeroable for Symbol {}
// SAFETY: Type is composed of simple primitive numerics.
unsafe impl bytemuck::AnyBitPattern for Symbol {}

impl Symbol {
    /// An index into the object file's symbol string table, which holds the character representations of the symbol names.
    pub const fn get_name_offset(&self) -> Option<usize> {
        match self.name_offset {
            0x0 => None,
            name_offset => Some(name_offset as usize),
        }
    }

    pub const fn get_type(&self) -> Type {
        Type::from_u8(self.info & 0xF)
    }

    pub const fn get_bind(&self) -> Bind {
        Bind::from_u8(self.info >> 4)
    }

    pub const fn get_visibility(&self) -> Visibility {
        Visibility::from_u8(self.visibility)
    }

    /// Gets the relevant section header table index. Some section indexes indicate special meanings.
    pub const fn get_section_header_index(&self) -> usize {
        self.section_header_index as usize
    }

    pub const fn get_value(&self) -> u64 {
        self.value
    }

    pub const fn get_size(&self) -> usize {
        self.size as usize
    }
}

impl core::fmt::Debug for Symbol {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Symbol")
            .field("Name Offset", &self.get_name_offset())
            .field("Type", &self.get_type())
            .field("Bind", &self.get_bind())
            .field("Visibility", &self.get_visibility())
            .field("Section Header Index", &self.get_section_header_index())
            .field("Value", &format_args!("{:#X}", self.get_value()))
            .field("Size", &format_args!("{:#X}", self.get_size()))
            .finish()
    }
}
