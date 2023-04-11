getrandom::register_custom_getrandom!(prng_custom_getrandom);
#[allow(clippy::unnecessary_wraps)]
fn prng_custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    for chunk in buf.chunks_mut(core::mem::size_of::<u64>()) {
        let rng_bytes = prng::next_u64().to_ne_bytes();
        chunk.copy_from_slice(&rng_bytes[..chunk.len()]);
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
                // Safety: ???
                unsafe {
                    let state_low = core::arch::x86_64::_rdtsc() as u128;
                    let state_high = core::arch::x86_64::_rdtsc() as u128;
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
