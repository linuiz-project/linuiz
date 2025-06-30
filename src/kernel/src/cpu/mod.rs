use core::{
    ops::Range,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use libsys::{Address, Frame};
use spin::{Mutex, Once};

pub mod interrupts;
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

/// Iterates the entries in the multiprocessing request, configuring and subsequently synchronizing
/// the other hardware threads in the system.
///
/// # Returns
///
/// - If request was satisfied, `Some` of the count of hardware threads in the system.
/// - If request was not satisfied, `None`.
pub fn begin_multiprocessing(mp_request: &limine::request::MpRequest) -> Option<usize> {
    let Some(response) = mp_request.get_response() else {
        warn!("Bootloader did not provide response to multiprocessing request.");
        return None;
    };

    debug!("Detecting and starting additional cores.");

    for cpu in response.cpus().iter().filter(|cpu| {
        // Make sure we skip the boot thread (we're using it right now!).
        cpu.lapic_id != response.bsp_lapic_id()
    }) {
        trace!(
            "Starting hardware thread: ID#{} LAPIC#{}",
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
            unsafe { synchronize(None) }
        }

        extern "C" fn _idle_forever(_: &limine::mp::Cpu) -> ! {
            crate::interrupts::halt_and_catch_fire()
        }

        if crate::params::use_multiprocessing() {
            cpu.goto_address.write(_mp_entry);
        } else {
            cpu.goto_address.write(_idle_forever);
        }
    }

    Some(response.cpus().len())
}

/// Frees bootloader reclaimable memory, then begins local post-memory-system-initialization
/// operations on each harware thread.
///
/// # Safety
///
/// - Function can only be run once at the end of the kernel init phase.
/// - `pre_call_sp` must be the current hardware thread's stack pointer immediately prior to
///   this method being called.
pub unsafe fn synchronize(
    bsp_requests: Option<(
        &limine::request::MpRequest,
        &limine::request::MemoryMapRequest,
    )>,
) -> ! {
    /// Checks if `range` contains the `stack_address`, and print out a message to
    /// indicate the check was true.
    fn check_range_contains_stack(range: &Range<usize>, stack_address: usize) -> bool {
        let range_contains_stack = range.contains(&stack_address);

        if range_contains_stack {
            trace!("Found boot stack: {range:#X?}");
        }

        range_contains_stack
    }

    /// Total count of hardware threads in the system.
    static HWTHREAD_COUNT: Once<usize> = Once::new();

    /// If `Some`, the current entry to be checked; if `None`, there are no more entries
    /// to check.
    static CHECK_ENTRY_IS_STACK: Once<Mutex<Option<Range<usize>>>> = Once::new();

    /// Total number of non-bootstrap processors that have checked whether the current
    /// entry contains its stack.
    static ENTRY_CHECK_DONE_COUNT: AtomicUsize = AtomicUsize::new(0);

    /// Indicates one of the non-bootstrap processors found that an entry contains its
    /// stack.
    static ENTRY_CHECK_CONSENSUS: AtomicBool = AtomicBool::new(false);

    /// Indicates the bootstrap processor has completed all sycnrhonization, and initialization
    /// can continue.
    static ALL_CORES_READY: AtomicBool = AtomicBool::new(false);

    let stack_address = get_stack_ptr().addr();

    // If this this the bootstrap processor context, the following requests will have been passed...
    if let Some((mp_request, memory_map_request)) = bsp_requests {
        // Begin multiprocessing and store the processor count to use in synchronization later.
        if let Some(hwthread_count) = crate::cpu::begin_multiprocessing(mp_request) {
            HWTHREAD_COUNT.call_once(|| hwthread_count);
            ENTRY_CHECK_DONE_COUNT.store(hwthread_count, Ordering::Release);
        }

        debug!("Reclaiming bootloader memory...");

        memory_map_request
            .get_response()
            .expect("bootloader did not provide a response to the memory map request")
            .entries()
            .iter()
            // We're only freeing bootloader reclaimable memory...
            .filter(|entry| {
                entry.entry_type == limine::memory_map::EntryType::BOOTLOADER_RECLAIMABLE
            })
            .map(|entry| {
                let entry_start = usize::try_from(entry.base).unwrap();
                let entry_end = usize::try_from(entry.base + entry.length).unwrap();

                entry_start..entry_end
            })
            .filter(|entry_range| {
                // Check if the entry contains the BSP stack, and if so, filter it
                // (check returned false, so invert and return true to avoid filtering).
                !check_range_contains_stack(&entry_range, stack_address)
            })
            .filter(|entry_range| {
                if CHECK_ENTRY_IS_STACK.is_completed() {
                    // If the check entry has already been set, then update it...

                    let check_entry = CHECK_ENTRY_IS_STACK.wait();

                    let mut check_entry = check_entry.lock();
                    *check_entry = Some(entry_range.clone());
                } else {
                    // If the check entry has not already been set, then set it...

                    CHECK_ENTRY_IS_STACK.call_once(|| Mutex::new(Some(entry_range.clone())));
                }

                // Reset the entry check count to signal a new iteration...
                ENTRY_CHECK_DONE_COUNT.store(0, Ordering::Release);

                // Wait for all other hardware threads to finish checking entry...
                while ENTRY_CHECK_DONE_COUNT.load(Ordering::Acquire) < *HWTHREAD_COUNT.wait() {
                    core::hint::spin_loop();
                }

                // If the consensus was a positive check, filter it.
                !ENTRY_CHECK_CONSENSUS.load(Ordering::Acquire)
            })
            // We'll flat_map each entry to physical memory range...
            .flat_map(|entry_range| entry_range)
            // Iterate page-size chunks...
            .step_by(libsys::page_size())
            // Map entry to physical page address...
            .map(|address| Address::<Frame>::new(address).unwrap())
            // Free the requisite physical frames...
            .for_each(|frame| crate::mem::pmm::PhysicalMemoryManager::free_frame(frame).unwrap());

        debug!("Bootloader memory reclaimed.");

        // Update core readiness to indicate non-bootstrap processors can continue their initialization.
        ALL_CORES_READY.store(true, Ordering::Release);
    } else {
        let entry = CHECK_ENTRY_IS_STACK.wait();

        loop {
            // Wait for the check done count to be zero, signalling to start again.
            while ENTRY_CHECK_DONE_COUNT.load(Ordering::Acquire) > 0 {
                core::hint::spin_loop();
            }

            let entry_range = entry.lock();

            // If the entry is `None`, then we're done checking entries.
            let Some(entry_range) = &*entry_range else {
                break;
            };

            if check_range_contains_stack(&entry_range, stack_address) {
                // Only update the consensus value if it was previously false.
                ENTRY_CHECK_CONSENSUS.store(true, Ordering::Release);
            }

            // Add current hardware thread to completed checks count.
            ENTRY_CHECK_DONE_COUNT.fetch_add(1, Ordering::AcqRel);
        }

        trace!("Waiting for bootstrap processor to signal ready...");

        while !ALL_CORES_READY.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
    }

    core::arch::breakpoint();

    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::structures::tss::TaskStateSegment::new_with_stacks().load();

    // Safety: Function is only run once, right here.
    unsafe {
        crate::cpu::state::LocalState::init(1000);
    }

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();

    // Safety: The hardware thread is ready to be scheduled with tasks.
    unsafe {
        crate::cpu::state::begin_scheduling();
    }

    // This interrupt wait loop is necessary to ensure the core can jump into the scheduler.
    crate::interrupts::wait_indefinite()
}

/// Gets the current hardware thread's stack pointer.
#[inline(always)]
pub fn get_stack_ptr() -> *const u8 {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::RSP::read()
    }
}
