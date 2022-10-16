#[cfg(target_arch = "riscv64")]
pub mod rv64;
#[cfg(target_arch = "x86_64")]
pub mod x64;

pub mod reexport {
    #[cfg(target_arch = "x86_64")]
    pub mod x86_64 {
        pub use x86_64::{PhysAddr, VirtAddr};
    }
}
