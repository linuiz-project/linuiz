#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrivilegeLevel {
    /// Kernel
    Ring0,
    /// Kernel drivers
    Ring1,
    /// User drivers
    Ring2,
    /// User mode
    Ring3,
}

impl PrivilegeLevel {
    pub const fn from(val: u64) -> Self {
        match val {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("invalid privilege level!"),
        }
    }

    pub const fn as_u8(&self) -> u8 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }

    pub const fn as_u16(&self) -> u16 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }

    pub const fn as_u32(&self) -> u32 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }

    pub const fn as_u64(&self) -> u64 {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }

    pub const fn as_usize(&self) -> usize {
        match self {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}
