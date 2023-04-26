pub mod klog;
pub mod task;

use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, Hash)]
pub enum Vector {
    KlogInfo = 0x100,
    KlogError = 0x101,
    KlogDebug = 0x102,
    KlogTrace = 0x103,

    TaskExit = 0x200,
    TaskYield = 0x201,
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
