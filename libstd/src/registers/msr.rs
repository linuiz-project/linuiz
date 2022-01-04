use crate::instructions::cpuid::{Features, FEATURES};

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum MSR {
    IA32_APIC_BASE = 0x1B,
    IA32_X2APIC_APICID = 0x2050,
    IA32_TSC = 0x10,
    IA32_TSC_ADJUST = 0x3B,
    IA32_TSC_AUX = 0x103,
    IA32_TSC_DEADLINE = 0x6E0,
    IA32_MPERF = 0xE7,
    IA32_APERF = 0xE8,
    IA32_EFER = 0xC0000080,
    IA32_FS_BASE = 0xC0000100,
    IA32_GS_BASE = 0xC0000101,
    PLATFORM_INFO = 0xCE,
    FSB_FREQ = 0xCD,
}

impl MSR {
    #[inline(always)]
    pub fn read(self) -> u64 {
        assert!(
            FEATURES.contains(Features::MSR),
            "CPU does not support use of model-specific registers."
        );

        unsafe { self.read_unchecked() }
    }

    pub unsafe fn read_unchecked(self) -> u64 {
        let value: u64;

        core::arch:: asm!(
            "rdmsr",
            "shl rdx, 32",   // Shift high value to high bits
            "or rdx, rax",   // Copy low value in
            in("ecx") self as u32,
            out("rdx") value,
            options(nostack, nomem)
        );

        value
    }

    pub unsafe fn write_bit(self, bit: usize, set: bool) {
        assert!(bit <= 64, "bit must be within u64");

        let bit_mask = 1 << bit;
        let set_bit = (set as u64) << bit;

        self.write((self.read() & bit_mask) | set_bit);
    }

    pub unsafe fn write_bits(self, range: core::ops::Range<usize>, value: u64) {
        assert!(range.end <= 64, "range must be within u64 bits");

        let mask = crate::U64_BIT_MASKS[range.end - range.start];
        assert_eq!(value & !mask, 0, "value must exist within range");

        let shifted_mask = mask << range.start;
        let shifted_value = value << range.start;

        self.write((self.read() & !shifted_mask) | shifted_value);
    }

    pub unsafe fn write(self, value: u64) {
        assert!(
            FEATURES.contains(Features::MSR),
            "CPU does not support use of model-specific registers"
        );

        self.write_unchecked(value);
    }

    pub unsafe fn write_unchecked(self, value: u64) {
        core::arch::asm!(
            "mov rdx, rax", // Move high value in
            "shr rdx, 32",  // Shift high value to edx bits
            "wrmsr",
            in("ecx") self as u32,
            in("rax") value,
            options(nostack, nomem)
        );
    }
}
