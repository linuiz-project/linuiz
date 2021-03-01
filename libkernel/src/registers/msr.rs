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
        use bit_field::BitField;
        self.write(*self.read().set_bit(bit, set));

        debug_assert_eq!(self.read().get_bit(bit), set);
    }

    pub unsafe fn write_bits(self, range: core::ops::Range<usize>, value: u64) {
        use bit_field::BitField;

        self.write(*self.read().set_bits(range.clone(), value));
        debug_assert_eq!(self.read().get_bits(range), value);
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
