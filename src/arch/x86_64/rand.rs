use crate::arch::x86_64::cpuid::{extended_feature_info, feature_info};

#[derive(Debug, Error)]
enum Error {
    #[error("random generation maxed out retries")]
    MaximumRetries,

    #[error("random generation method not supported")]
    NotSupported,
}

const MAXIMUM_RETRIES: usize = 10000;

fn try_generate_rdseed() -> Result<u64, Error> {
    if !extended_feature_info().is_some_and(|cpuid| cpuid.has_rdseed()) {
        return Err(Error::NotSupported);
    }

    let mut value = 0u64;
    let mut iterations = 0usize;

    loop {
        if iterations > MAXIMUM_RETRIES {
            return Err(Error::MaximumRetries);
        }

        // Safety: Feature is checked to exist.
        match unsafe { core::arch::x86_64::_rdseed64_step(&mut value) } {
            0 => {
                iterations += 1;
                core::hint::spin_loop();
            }

            1 => {
                return Ok(value);
            }

            _ => unreachable!(),
        }
    }
}

fn try_generate_rdrand() -> Result<u64, Error> {
    if !feature_info().is_some_and(|cpuid| cpuid.has_rdrand()) {
        return Err(Error::NotSupported);
    }

    let mut value = 0u64;
    let mut iterations = 0usize;

    loop {
        if iterations > MAXIMUM_RETRIES {
            return Err(Error::MaximumRetries);
        }

        // Safety: Feature is checked to exist.
        match unsafe { core::arch::x86_64::_rdrand64_step(&mut value) } {
            0 => {
                iterations += 1;
                core::hint::spin_loop();
            }

            1 => {
                return Ok(value);
            }

            _ => unreachable!(),
        }
    }
}

fn try_generate_rdtsc() -> Result<u64, Error> {
    if !feature_info().is_some_and(|cpuid| cpuid.has_tsc()) {
        return Err(Error::NotSupported);
    }

    // Safety: Feature is checked to exist.
    let tsc = unsafe { core::arch::x86_64::_rdtsc() };

    Ok(tsc)
}

pub fn generate_random() -> u64 {
    try_generate_rdseed()
        .or_else(|_| try_generate_rdrand())
        .or_else(|_| try_generate_rdtsc())
        .expect("could not generate a random number")
}
