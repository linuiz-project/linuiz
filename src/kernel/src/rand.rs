use core::mem::MaybeUninit;

#[no_mangle]
unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), getrandom::Error> {
    let buf = core::slice::from_raw_parts_mut(
        // `dest` may be uninitialized for `len`
        dest.cast::<MaybeUninit<u8>>(),
        len,
    );

    trace!("[RAND] BUFFER LEN: {}", buf.len());

    for (index, chunk) in buf.chunks_mut(core::mem::size_of::<u64>()).enumerate() {
        let rng_bytes = prng::next_u64().to_ne_bytes();

        trace!("[RAND] CHUNK#{}: {:?}", index, rng_bytes);

        chunk.write_copy_of_slice(&rng_bytes[..chunk.len()]);
    }

    Ok(())
}

pub mod prng {
    use rand_core::RngCore;
    use rand_pcg::Pcg64Mcg;
    use spin::{Lazy, Mutex};

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
