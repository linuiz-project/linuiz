/// Base MSR address used by the x2APIC.
const BASE_MSR_ADDR: u32 = 0x800;

/// x2APIC implementation of the [super::APIC] trait.
pub struct x2APIC;

unsafe impl Send for x2APIC {}
unsafe impl Sync for x2APIC {}

impl super::APIC for x2APIC {
    #[inline]
    unsafe fn read_register(&self, register: super::Register) -> u64 {
        crate::registers::msr::rdmsr(BASE_MSR_ADDR + (register as u32))
    }

    #[inline]
    unsafe fn write_register(&self, register: super::Register, value: u64) {
        crate::registers::msr::wrmsr(BASE_MSR_ADDR + (register as u32), value);
    }
}
