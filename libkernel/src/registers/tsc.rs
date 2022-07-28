pub struct TSC;

impl TSC {
    #[inline(always)]
    pub fn read() -> u64 {
        let value: u64;

        unsafe {
            core::arch::asm!(
                "push rax",
                "rdtsc",
                "shl rdx, 32",
                "or rdx, rax",
                "pop rax",
                out("rdx") value,
                options(nostack, nomem)
            )
        };

        value
    }
}
