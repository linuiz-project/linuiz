use crate::memory::MMIO;

/// Base address of the xAPIC MMIO memory.
pub const BASE_PTR: *mut u64 = 0xFEE00000 as *mut u64;

/// xAPIC implementation of the [super::APIC] trait.
pub struct xAPIC(pub MMIO);

unsafe impl Send for xAPIC {}
unsafe impl Sync for xAPIC {}

impl super::APIC for xAPIC {
    #[inline]
    unsafe fn read_register(&self, register: super::Register) -> u64 {
        let register_offset = (register as usize) << 4;
        let register_ptr = BASE_PTR.add(register_offset);

        // This is MMIO, so ensure reads are volatile.
        register_ptr.read_volatile()
    }

    #[inline]
    unsafe fn write_register(&self, register: super::Register, value: u64) {
        let register_offset = (register as usize) << 4;
        let register_ptr = BASE_PTR.add(register_offset);

        // This is MMIO, so ensure writes are volatile.
        register_ptr.write_volatile(value);
    }
}
