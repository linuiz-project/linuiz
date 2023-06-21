pub mod state;

pub fn read_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::get_cpu_id()
    }
}
