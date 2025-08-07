#[repr(transparent)]
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AddressSpaceId(usize);

impl AddressSpaceId {
    const MAX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 0b1111_1111_1111 }
            _ => { todo!() }
        }
    };

    pub const KERNEL: Self = Self(0);

    pub fn new(id: usize) -> Option<Self> {
        (id <= Self::MAX).then_some(Self(id))
    }

    /// Gets the process context ID.
    pub fn get(&self) -> usize {
        self.0
    }
}
