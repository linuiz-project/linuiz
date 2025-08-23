use crate::{cpu::local_state::LocalState, mem::KernelMapper, params::KernelParameters};

pub mod local_state;

pub fn get_id() -> u32 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::get_processor_id()
    }
}

/// # Safety
///
/// This function has the potential to modify state in such a way as to disrupt
/// software execution. It should be run only once per processor at the very
/// beginning of code execution.
pub unsafe fn configure() {
    #[cfg(target_arch = "x86_64")]
    // Safety: Caller is required to maintain safety invariants.
    unsafe {
        crate::arch::x86_64::configure_processor();
    }
}

/// Iterates the entries in the multiprocessing request, configuring and
/// subsequently synchronizing the other processors in the system.
///
/// # Returns
///
/// - If request was satisfied, `Some` of the count of non-bootstrap processor
///   in the system.
/// - If request was not satisfied, `None`.
pub fn begin_multiprocessing(mp_request: &limine::request::MpRequest) -> Option<usize> {
    let Some(response) = mp_request.get_response() else {
        warn!("Bootloader did not provide response to multiprocessing request.");
        return None;
    };

    debug!("Detecting and starting additional cores.");

    response
        .cpus()
        .iter()
        .filter(|cpu| cpu.lapic_id != response.bsp_lapic_id())
        .for_each(|cpu| {
            trace!("Starting processor: ID#{} LAPIC#{}", cpu.id, cpu.lapic_id);

            if KernelParameters::use_multiprocessing() {
                extern "C" fn _mp_entry(_: &limine::mp::Cpu) -> ! {
                    // Safety: Function is run only once for this processor.
                    unsafe {
                        configure();
                    }

                    // Safety: All currently referenced memory should also be
                    //         mapped in the kernel page tables.
                    unsafe {
                        KernelMapper::swap_into();
                    }

                    // Safety: processor still in init phase.
                    unsafe { start(None, None) }
                }

                cpu.goto_address.write(_mp_entry);
            } else {
                extern "C" fn _idle_forever(_: &limine::mp::Cpu) -> ! {
                    crate::cpu::halt_and_catch_fire()
                }

                cpu.goto_address.write(_idle_forever);
            }
        });

    Some(response.cpus().len())
}

/// Frees bootloader reclaimable memory, then begins local
/// post-memory-system-initialization operations on each harware thread.
///
/// # Safety
///
/// - Function should only be run once at the end of the kernel init phase.
#[allow(clippy::too_many_lines)]
pub unsafe fn start(
    mp_request: Option<&limine::request::MpRequest>,
    _memory_map_request: Option<&limine::request::MemoryMapRequest>,
) -> ! {
    mp_request
        .and_then(begin_multiprocessing)
        .inspect(|processor_count| {
            trace!("Detected {processor_count} processors.");
        });

    debug!("Preparing for task scheduling...");

    LocalState::init();

    core::arch::breakpoint();

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();

    // // Safety: The processor is ready to be scheduled with tasks.
    // unsafe {
    //     crate::cpu::local_state::begin_scheduling();
    // }

    // This interrupt wait loop is necessary to ensure the core can jump into the
    // scheduler.
    crate::interrupts::wait_indefinite()
}

/// Gets the current processor's stack pointer.
#[inline(always)]
pub fn get_stack_ptr() -> *const u8 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::RSP::read()
    }
}

/// Murder—in cold electrons—the current processor.
#[inline(never)]
pub fn halt_and_catch_fire() -> ! {
    crate::interrupts::disable();

    crate::interrupts::wait_indefinite()
}
