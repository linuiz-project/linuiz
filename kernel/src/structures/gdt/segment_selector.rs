use crate::PrivilegeLevel;
use bit_field::BitField;

#[repr(C)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SegmentSelector(pub u16);

impl SegmentSelector {
    pub const fn new(index: u16, rpl: PrivilegeLevel) -> Self {
        Self {
            0: index << 3 | (rpl as u16),
        }
    }

    pub fn index(self) -> u16 {
        self.0 >> 3
    }

    pub fn rpl(self) -> PrivilegeLevel {
        PrivilegeLevel::from(self.0.get_bits(0..2))
    }

    pub fn set_rpl(&mut self, rpl: PrivilegeLevel) {
        self.0.set_bits(0..2, rpl as u16);
    }
}

impl core::fmt::Debug for SegmentSelector {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SegmentSelector")
            .field("Index", &self.index())
            .field("RPL", &self.rpl())
            .finish()
    }
}
