use x86_64::PhysAddr;

pub struct Frame(u64);

impl Frame {
    pub fn from_addr(addr: u64) -> Self {
        assert_eq!(addr % 0x1000, 0, "address must be page-aligned");
        Self { 0: addr / 0x1000 }
    }

    pub fn from_index(index: u64) -> Self {
        Self { 0: index }
    }

    pub fn index(&self) -> u64 {
        self.0
    }

    pub fn addr(&self) -> PhysAddr {
        PhysAddr::new(self.0 * 0x1000)
    }

    pub unsafe fn clear(&mut self) {
        core::ptr::write_bytes((self.0 * 0x1000) as *mut u8, 0x0, 0x1000);
    }
}

impl core::fmt::Debug for Frame {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Frame")
            .field("Index", &self.index())
            .field("Address", &self.addr())
            .finish()
    }
}
