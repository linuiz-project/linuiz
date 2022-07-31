/// Base address of the xAPIC MMIO memory.
const xAPIC_BASE_PTR: *mut u64 = 0xFEE00000 as *mut u64;

/// xAPIC implementation of the [super::APIC] trait.
pub struct xAPIC;

impl super::APIC for xAPIC {
    #[inline]
    unsafe fn read_register(&self, register: super::Register) -> u64 {
        let register_offset = (register as usize) << 4;
        let register_ptr = xAPIC_BASE_PTR.add(register_offset);

        // This is MMIO, so ensure reads are volatile.
        register_ptr.read_volatile()
    }

    #[inline]
    unsafe fn write_register(&self, register: super::Register, value: u64) {
        let register_offset = (register as usize) << 4;
        let register_ptr = xAPIC_BASE_PTR.add(register_offset);

        // This is MMIO, so ensure writes are volatile.
        register_ptr.write_volatile(value);
    }
}
