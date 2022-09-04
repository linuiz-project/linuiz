use core::convert::TryFrom;
use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
enum SyscallVector {
    Test = 0x0,
}

/// Handler for executing system calls from userspace.
///
/// REMARK: This function follows the System V ABI on x86_64.
pub fn syscall_handler(rdi: u64, rsi: u64, rdx: u64, rcx: u64, r8: u64, r9: u64) -> u64 {
    match SyscallVector::try_from(rdi) {
        Ok(vector) => info!("Syscall call: {:#X?}", vector),

        Err(error) => warn!("Unhandled system call vector: {:?}", error),
    }

    0
}
