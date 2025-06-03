pub mod klog;
pub mod task;

use core::ffi::c_void;
use num_enum::TryFromPrimitive;

#[repr(usize)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, Hash)]
pub enum Vector {
    KlogInfo = 0x100,
    KlogError = 0x101,
    KlogDebug = 0x102,
    KlogTrace = 0x103,

    TaskExit = 0x200,
    TaskYield = 0x201,
}

const_assert!({
    use core::mem::size_of;
    size_of::<Result>() <= size_of::<(u64, u64)>()
});

pub type Result = core::result::Result<Success, Error>;

pub trait ResultConverter {
    type Registers;

    fn from_registers(regs: Self::Registers) -> Self;
    fn into_registers(self) -> Self::Registers;
}

impl ResultConverter for Result {
    type Registers = (usize, usize);

    fn from_registers((discriminant, value): Self::Registers) -> Self {
        let discriminant = u32::try_from(discriminant).unwrap();
        match Error::try_from_primitive(discriminant).map_err(|err| err.number) {
            Ok(err) => Err(err),

            Err(0x0) => Ok(Success::Ok),
            Err(0x1) => Ok(Success::Ptr(value as *mut c_void)),
            Err(0x2) => Ok(Success::NonNullPtr(
                core::ptr::NonNull::new(value as *mut c_void).unwrap(),
            )),

            Err(_) => unimplemented!(),
        }
    }

    fn into_registers(self) -> Self::Registers {
        match self {
            Ok(success @ Success::Ok) => (success.discriminant() as usize, usize::default()),
            Ok(success @ Success::Ptr(ptr)) => (success.discriminant() as usize, ptr.addr()),
            Ok(success @ Success::NonNullPtr(ptr)) => {
                (success.discriminant() as usize, ptr.addr().get())
            }

            Err(err) => (err as usize, Default::default()),
        }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Success {
    Ok = 0x0,
    Ptr(*mut c_void) = 0x1,
    NonNullPtr(core::ptr::NonNull<c_void>) = 0x2,
}

impl Success {
    #[inline]
    const fn discriminant(&self) -> u32 {
        // Safety: discrimnent is guaranteed to be the first bytes
        unsafe { *(self as *const Self as *const u32) }
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum Error {
    InvalidVector = 0x10000,
    InvalidPtr = 0x20000,
    InvalidUtf8 = 0x30000,

    UnmappedMemory = 0x40000,

    NoActiveTask = 0x50000,
}

impl From<core::str::Utf8Error> for Error {
    fn from(_: core::str::Utf8Error) -> Self {
        Self::InvalidUtf8
    }
}
