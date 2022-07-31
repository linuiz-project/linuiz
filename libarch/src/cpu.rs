/// Gets the ID of the current core.
pub fn get_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::instructions::x86_64::cpuid::exec;

        if let Some(registers) =
            // IA32 SDM instructs to enumerate this leaf first...
            exec(0x1F, 0x0)
                // ... this leaf second ...
                .or_else(|| exec(0xB, 0x0))
        {
            registers.edx()
        } else if let Some(registers) =
            // ... and finally, this leaf as an absolute fallback.
            exec(0x1, 0x0)
        {
            registers.ebx() >> 24
        } else {
            // TODO possibly don't just panic and fail here?
            panic!("CPUID ID enumeration failed.")
        }
    }
}
