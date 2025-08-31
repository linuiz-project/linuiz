#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(usize);

impl AddressSpaceId {
    const MAX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 0b1111_1111_1111 }
            _ => { unimplemented!() }
        }
    };

    pub const KERNEL: Self = Self(0);

    pub fn new(id: usize) -> Option<Self> {
        (id <= Self::MAX).then_some(Self(id))
    }
}

impl From<AddressSpaceId> for usize {
    fn from(value: AddressSpaceId) -> Self {
        value.0
    }
}
