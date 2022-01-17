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

// UNSAFETY: It is *possible* that the current CPU doesn't support the MSR
//           feature. In this case, well... all of this fails. And we're
//           going to ignore that. :)
impl MSR {
    #[inline(always)]
    pub fn read(self) -> u64 {
        unsafe {
            let value: u64;

            core::arch:: asm!(
                "push rax",     // Preserve the `rax` value.
                "rdmsr",
                "shl rdx, 32",  // Shift high value to high bits
                "or rdx, rax",  // Copy low value in
                "pop rax",
                in("ecx") self as u32,
                out("rdx") value,
                options(nostack, nomem)
            );

            value
        }
    }

    #[inline(always)]
    pub unsafe fn write(self, value: u64) {
        core::arch::asm!(
            "wrmsr",
            in("ecx") self as u32,
            in("rax") value,
            in("rdx") value >> 32,
            options(nostack, nomem)
        );
    }
}
