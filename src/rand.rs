#[unsafe(no_mangle)]
#[allow(clippy::unnecessary_wraps)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dst: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    (0..len)
        .step_by(size_of::<u64>())
        .try_for_each(|chunk_offset| {
            let rng_bytes = prng::next_u64().to_ne_bytes();
            let chunk_size = usize::min(len - chunk_offset, size_of::<u64>());

            // Safety:
            //  - `rng_bytes` is on the local stack, `dest` should not be (so cannot
            //    overlap).
            //  - `dest` is valid as `u8` for `len`, so can be written to as raw bytes.
            unsafe {
                core::ptr::copy_nonoverlapping(
                    rng_bytes.as_ptr(),
                    dst.byte_add(chunk_offset),
                    chunk_size,
                );
            }

            Ok(())
        })
}

pub mod prng {
    use crate::util::sync::{Lazy, Mutex};
    use rand_pcg::{Pcg64Mcg, rand_core::RngCore};

    static PCG: Lazy<Mutex<Pcg64Mcg>> = Lazy::new(|| {
        let (seed_low, seed_high) = {
            cfg_select! {
                target_arch = "x86_64" => {
                    use crate::arch::x86_64::rand::generate_random;
                    (generate_random(), generate_random())
                }

                _ => { unimplemented!() }
            }
        };

        let state_seed = (u128::from(seed_high) << u64::BITS) | u128::from(seed_low);
        let prng = Pcg64Mcg::new(state_seed);

        Mutex::new(prng)
    });

    pub fn next_u32() -> u32 {
        PCG.with_lock(Pcg64Mcg::next_u32)
    }

    pub fn next_u64() -> u64 {
        PCG.with_lock(Pcg64Mcg::next_u64)
    }
}
