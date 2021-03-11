#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum MSR {
    IA32_APIC_BASE = 0x1B,
    IA32_X2APIC_APICID = 2050,
}

impl MSR {
    pub fn read(self) -> u64 {
        assert!(
            crate::instructions::cpu_features().contains(crate::instructions::CPUFeatures::MSR),
            "CPU does not support use of model-specific registers"
        );

        let low: u64;
        let high: u64;

        unsafe {
            asm!(
                "mov ecx, {:e}",
                "rdmsr",
                "mov {}, rax",
                "mov {}, rdx",
                in(reg) self as u32,
                out(reg) low,
                out(reg) high,
                options(nomem)
            );
        }

        (high << 32) | low
    }

    pub unsafe fn write_bit(self, bit: usize, set: bool) {
        assert!(bit <= 64, "bit must be within u64");

        let bit_mask = 1 << bit;
        let set_bit = (set as u64) << bit;

        self.write((self.read() & bit_mask) | set_bit);

        debug_assert_eq!(self.read() & bit_mask, set_bit);
    }

    pub unsafe fn write_bits(self, range: core::ops::Range<usize>, value: u64) {
        assert!(range.end <= 64, "range must be within u64 bits");

        let mask = crate::U64_BIT_MASKS[range.end - range.start];
        assert_eq!(value & !mask, 0, "value must exist within range");

        let shifted_mask = mask << range.start;
        let shifted_value = value << range.start;

        self.write((self.read() & !shifted_mask) | shifted_value);

        debug_assert_eq!(self.read() & shifted_mask, shifted_value);
    }

    pub unsafe fn write(self, value: u64) {
        assert!(
            crate::instructions::cpu_features().contains(crate::instructions::CPUFeatures::MSR),
            "CPU does not support use of model-specific registers"
        );

        asm!(
            "mov ecx, {:e}",
            "mov eax, {:e}",
            "mov edx, {:e}",
            "wrmsr",
            in(reg) self as u32,
            in(reg) value as u32,
            in(reg) (value >> 32) as u32,
            options(nomem)
        );
    }
}
