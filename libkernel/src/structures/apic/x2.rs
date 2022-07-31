/// Base MSR address used by the x2APIC.
const x2APIC_BASE_MSR_ADDR: u32 = 0x800;

/// x2APIC implementation of the [super::APIC] trait.
pub struct x2APIC;

impl super::APIC for x2APIC {
    #[inline]
    unsafe fn read_register(&self, register: super::Register) -> u64 {
        crate::registers::msr::rdmsr(x2APIC_BASE_MSR_ADDR + (register as u32))
    }

    #[inline]
    unsafe fn write_register(&self, register: super::Register, value: u64) {
        crate::registers::msr::wrmsr(x2APIC_BASE_MSR_ADDR + (register as u32), value);
    }
}
