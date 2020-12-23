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

impl From<PrivilegeLevel> for u8 {
    fn from(val: PrivilegeLevel) -> Self {
        match val {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}

impl From<PrivilegeLevel> for u16 {
    fn from(val: PrivilegeLevel) -> Self {
        match val {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}

impl From<PrivilegeLevel> for u64 {
    fn from(val: PrivilegeLevel) -> Self {
        match val {
            PrivilegeLevel::Ring0 => 0,
            PrivilegeLevel::Ring1 => 1,
            PrivilegeLevel::Ring2 => 2,
            PrivilegeLevel::Ring3 => 3,
        }
    }
}

impl From<u8> for PrivilegeLevel {
    fn from(val: u8) -> Self {
        match val {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("invalid privilege level!"),
        }
    }
}

impl From<u16> for PrivilegeLevel {
    fn from(val: u16) -> Self {
        match val {
            0 => PrivilegeLevel::Ring0,
            1 => PrivilegeLevel::Ring1,
            2 => PrivilegeLevel::Ring2,
            3 => PrivilegeLevel::Ring3,
            _ => panic!("invalid privilege level!"),
        }
    }
}
