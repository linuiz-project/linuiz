mod contexts;
mod setup;
mod syscall;

pub use contexts::*;
pub use setup::*;
pub use syscall::*;

pub fn read_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::get_cpu_id()
    }
}
