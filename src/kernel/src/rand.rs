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
            //  - `rng_bytes` is on the local stack, `dest` should not be (so cannot overlap).
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
    use rand_pcg::{Pcg64Mcg, rand_core::RngCore};
    use spin::{Lazy, Mutex};

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    fn produce_seed() -> u64 {
        todo!()
    }

    static PCG: Lazy<Mutex<Pcg64Mcg>> = Lazy::new(|| {
        Mutex::new(Pcg64Mcg::new({
            #[cfg(target_arch = "x86_64")]
            {
                // Safety: `_rdtsc` isn't unsafe, so far as I can tell.
                unsafe {
                    let state_low = u128::from(core::arch::x86_64::_rdtsc());

                    // spin for a random-ish length to allow timestamp counter to progress
                    for _ in 0..(state_low & 0xFF) {
                        core::hint::spin_loop();
                    }

                    let state_high = u128::from(core::arch::x86_64::_rdtsc());

                    state_low | (state_high << 64)
                }
            }
        }))
    });

    pub fn next_u32() -> u32 {
        PCG.lock().next_u32()
    }

    pub fn next_u64() -> u64 {
        PCG.lock().next_u64()
    }
}
