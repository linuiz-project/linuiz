use core::arch::asm;
use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, Hash)]
pub enum Vector {
    SyslogInfo = 0x100,
    SyslogError = 0x101,
    SyslogDebug = 0x102,
    SyslogTrace = 0x103,
}

#[repr(C)]
#[derive(Debug)]
pub enum Result {
    Ok,
    InvalidPtr(*const u8),
    Utf8Error,
    IntError,

    InvalidVector,
}

impl core::ops::FromResidual<Self> for Result {
    fn from_residual(residual: Self) -> Self {
        residual
    }
}

impl core::ops::FromResidual<core::result::Result<core::convert::Infallible, Self>> for Result {
    fn from_residual(residual: core::result::Result<core::convert::Infallible, Self>) -> Self {
        unsafe { residual.unwrap_err_unchecked() }
    }
}

impl core::ops::Try for Result {
    type Output = Self;
    type Residual = Self;

    fn from_output(output: Self::Output) -> Self {
        output
    }

    fn branch(self) -> core::ops::ControlFlow<Self::Residual, Self::Output> {
        match self {
            Self::Ok => core::ops::ControlFlow::Continue(self),
            err => core::ops::ControlFlow::Break(err),
        }
    }
}

impl From<core::num::TryFromIntError> for Result {
    fn from(_: core::num::TryFromIntError) -> Self {
        Self::IntError
    }
}

impl From<core::str::Utf8Error> for Result {
    fn from(_: core::str::Utf8Error) -> Self {
        Self::Utf8Error
    }
}

pub fn syslog_info(str: &str) -> Result {
    let str_ptr = str.as_ptr();
    let str_len = str.len();

    let low: u64;
    let high: u64;

    unsafe {
        asm!(
            "syscall",
            in("rdi") str_ptr,
            in("rsi") str_len,
            out("rax") low,
            out("rdx") high,
            // callee saved registers
            out("rcx") _,
            out("r8") _,
            out("r9") _,
            out("r11") _,
            options(nostack, nomem)
        )
    }

    let result = ((high as u128) << u64::BITS) | (low as u128);
    unsafe { core::mem::transmute(result) }
}
