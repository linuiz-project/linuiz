#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum MSR {
    IA32_APIC_BASE = 0x1B,
    IA32_X2APIC_APICID = 2050,
}

impl MSR {
    pub fn read(self) -> u64 {
        if !crate::instructions::cpuid_features().contains(crate::instructions::CPUFeatures::MSR) {
            panic!("CPU does not support use of model-specific registers");
        }

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

    pub unsafe fn write(self, value: u64) {
        if !crate::instructions::cpuid_features().contains(crate::instructions::CPUFeatures::MSR) {
            panic!("CPU does not support use of model-specific registers");
        }

        let low = value as u32;
        let high = (value >> 32) as u32;

        asm!(
            "mov ecx, {:e}",
            "mov {:e}, eax",
            "mov {:e}, edx",
            "wrmsr",
            in(reg) self as u32,
            in(reg) low,
            in(reg) high,
            options(nomem)
        );
    }
}
