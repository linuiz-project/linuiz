#[cfg(target_arch = "x86_64")]
mod x64;
#[cfg(target_arch = "x86_64")]
pub use x64::*;

#[cfg(target_arch = "riscv64")]
mod rv64;
#[cfg(target_arch = "riscv64")]
pub use rv64::*;
