pub struct Error;

pub type Result = core::result::Result<u64, Error>;

pub fn rand() -> Result {
    #[cfg(target_arch = "x86_64")]
    {
        let mut rand = 0;
        // ### Safety: It's an intrinsic.
        match unsafe { core::arch::x86_64::_rdrand64_step(&mut rand) } {
            1 => Ok(rand),
            0 => Err(Error),
            // ### Safety: Function guarantees a value of 0 or 1.
            _ => unsafe { core::hint::unreachable_unchecked() },
        }
    }
}
