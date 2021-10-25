#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(non_camel_case_types)]
pub enum MSR {
    IA32_APIC_BASE = 0x1B,
    IA32_X2APIC_APICID = 2050,
    IA32_EFER = 0xC0000080,
    IA32_FS_BASE = 0xC0000100,
    IA32_GS_BASE = 0xC0000101,
}

impl MSR {
    #[inline]
    pub fn read(self) -> u64 {
        assert!(
            crate::instructions::cpu_features().contains(crate::instructions::CPUFeatures::MSR),
            "CPU does not support use of model-specific registers"
        );

        let value: u64;

        unsafe {
            asm!(
                "mov ecx, {:e}",
                "rdmsr",
                "mov r8, rdx",  // Move high value in
                "shl r8, 32",   // Shift high value to high bits
                "or r8, rax",   // Copy low value in
                in(reg) self as u32,
                out("r8") value,
                options(nostack, nomem)
            );
        }

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
            crate::instructions::cpu_features().contains(crate::instructions::CPUFeatures::MSR),
            "CPU does not support use of model-specific registers"
        );

        asm!(
            "mov ecx, {:e}",
            "mov rax, r8",  // Move low value in
            "mov rdx, r8",  // Move high value in
            "shr rdx, 32",  // Shift high value to edx bits
            "wrmsr",
            in(reg) self as u32,
            in("r8") value,
            options(nostack, nomem)
        );
    }
}
