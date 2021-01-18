use x86_64::VirtAddr;

#[repr(transparent)]
pub struct Page(u64);

impl Page {
    pub fn from_addr(virt_addr: VirtAddr) -> Self {
        let addr_u64 = virt_addr.as_u64();
        assert_eq!(
            addr_u64 & !0x000FFFFF_FFFFF000,
            0,
            "frame address format is invalid: {:?}",
            virt_addr
        );
        Self {
            0: addr_u64 / 0x1000,
        }
    }

    pub fn addr(&self) -> VirtAddr {
        VirtAddr::new(self.0 * 0x1000)
    }
}

impl core::fmt::Debug for Page {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.debug_tuple("Page").field(&self.addr()).finish()
    }
}
