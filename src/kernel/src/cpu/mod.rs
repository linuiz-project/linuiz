pub mod state;

pub fn get_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::get_hwthread_id()
    }
}

/// # Safety
///
/// This function has the potential to modify state in such a way as to disrupt
/// software execution. It should be run only once per hardware thread at the very
/// beginning of code execution.
pub unsafe fn configure() {
    // Safety: Caller is required to meet invariants.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::configure_hwthread();
    }
}

pub fn start_mp(mp_request: &limine::request::MpRequest) {
    let Some(response) = mp_request.get_response() else {
        warn!("Bootloader did not provide response to multiprocessing request.");
        return;
    };

    debug!("Detecting and starting additional cores.");

    for cpu in response.cpus().iter().filter(|cpu| {
        // Make sure we skip the boot thread (we're using it right now!).
        cpu.lapic_id != response.bsp_lapic_id()
    }) {
        trace!(
            "Starting hardware thread: ID ID#{} LAPIC#{}",
            cpu.id, cpu.lapic_id
        );

        extern "C" fn _mp_entry(_: &limine::mp::Cpu) -> ! {
            // Safety: Function is run only once for this hardware thread.
            unsafe {
                configure();
            }

            // Safety: All currently referenced memory should also be mapped in the kernel page tables.
            crate::mem::with_kernel_mapper(|kmapper| unsafe {
                kmapper.swap_into();
            });

            // Safety: Hardware thread still in init phase.
            unsafe { run() }
        }

        cpu.goto_address.write(_mp_entry);
    }
}

/// # Safety
///
/// - Function can only be run once at the end of the kernel init phase.
pub unsafe fn run() -> ! {
    crate::cpu::state::init(1000);

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();
    crate::cpu::state::begin_scheduling();

    // This interrupt wait loop is necessary to ensure the core can jump into the scheduler.
    crate::interrupts::wait_indefinite()
}
