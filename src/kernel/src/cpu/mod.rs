mod setup;
pub use setup::*;

pub mod syscall;

pub fn read_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::get_cpu_id()
    }
}
