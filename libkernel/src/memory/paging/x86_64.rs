bitflags::bitflags! {
    #[repr(transparent)]
    pub struct PageAttributes: u64 {
        const PRESENT = 1 << 0;
        const WRITABLE = 1 << 1;
        const USERSPACE = 1 << 2;
        const WRITE_THROUGH = 1 << 3;
        const UNCACHEABLE = 1 << 4;
        const ACCESSED = 1 << 5;
        const DIRTY = 1 << 6;
        // We don't support huge pages for now.
        // const HUGE_PAGE = 1 << 7;
        const GLOBAL = 1 << 8;
        //  9..=11 available
        // 12..52 frame index
        // 52..=58 available
        const NO_EXECUTE = 1 << 63;

        const RO = Self::PRESENT.bits() | Self::NO_EXECUTE.bits();
        const RW = Self::PRESENT.bits() | Self::WRITABLE.bits() | Self::NO_EXECUTE.bits();
        const RX = Self::PRESENT.bits();
        const MMIO = Self::RW.bits() | Self::UNCACHEABLE.bits();
    }
}
