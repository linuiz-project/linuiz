use crate::{
    cpu::local_state::LocalState,
    mem::{KernelMapper, pmm::PhysicalMemoryManager},
    params::KernelParameters,
    task::asid::AddressSpaceId,
};
use core::{
    ops::Range,
    sync::atomic::{Atomic, Ordering},
};
use libsys::{
    address::{Address, Frame},
    constants::page_size,
};
use spin::{Barrier, Once, RwLock};

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
/// software execution. It should be run only once per hardware thread at the
/// very beginning of code execution.
pub unsafe fn configure() {
    // Safety: Caller is required to meet invariants.
    unsafe {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x86_64::configure_hwthread();
    }
}

/// Iterates the entries in the multiprocessing request, configuring and
/// subsequently synchronizing the other hardware threads in the system.
///
/// # Returns
///
/// - If request was satisfied, `Some` of the count of non-bootstrap hardware
///   threads in the system.
/// - If request was not satisfied, `None`.
pub fn begin_multiprocessing(mp_request: &limine::request::MpRequest) -> Option<usize> {
    #[unsafe(naked)]
    extern "C" fn _mp_entry(_: &limine::mp::Cpu) -> ! {
        unsafe extern "sysv64" fn main(stack_ptr: *const u8) -> ! {
            // Safety: Function is run only once for this hardware thread.
            unsafe {
                configure();
            }

            KernelMapper::with(|kernel_mapper| {
                // Safety: All currently referenced memory should also be mapped in the kernel
                // page tables.
                unsafe {
                    kernel_mapper.swap_into(AddressSpaceId::KERNEL);
                }
            });

            // Safety: Hardware thread still in init phase.
            unsafe { synchronize(stack_ptr, None) }
        }

        // Move the exact starting stack pointer into the first parameter.
        // This will be used in `crate::cpu::synchronize` to determine which memory
        // segments are stack spaces used by the kernel setup process.
        core::arch::naked_asm!(
            "
            mov rdi, rsp
            call {}
            ",
            sym main
        )
    }

    extern "C" fn _idle_forever(_: &limine::mp::Cpu) -> ! {
        crate::cpu::halt_and_catch_fire()
    }

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
            trace!(
                "Starting hardware thread: ID#{} LAPIC#{}",
                cpu.id, cpu.lapic_id
            );

            if KernelParameters::use_multiprocessing() {
                cpu.goto_address.write(_mp_entry);
            } else {
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
/// - Function can only be run once at the end of the kernel init phase.
/// - `pre_call_sp` must be the current hardware thread's stack pointer
///   immediately prior to this method being called.
#[allow(clippy::too_many_lines)]
pub unsafe fn synchronize(
    stack_ptr: *const u8,
    bsp_requests: Option<(
        &limine::request::MpRequest,
        &limine::request::MemoryMapRequest,
    )>,
) -> ! {
    /// Checks if `range` contains the `stack_address`, and print out a message
    /// to indicate the check was true.
    fn check_range_contains_stack(range: &Range<usize>, stack: &Range<usize>) -> bool {
        let range_contains_stack =
            core::cmp::max(range.start, stack.start) <= core::cmp::min(range.end, stack.end);

        if range_contains_stack {
            trace!("Checked: {range:#X?} contains {stack:#X?}");
        }

        range_contains_stack
    }

    let stack_address = stack_ptr.addr();
    let stack_range = (stack_address - crate::KERNEL_STACK_SIZE)..stack_address;
    trace!("Stack: {stack_range:#X?}");

    static ENTRY_CHECK: RwLock<Option<(Range<usize>, Atomic<bool>)>> = RwLock::new(None);
    static ENTRY_READY_SYNC: Once<Barrier> = Once::new();
    static ENTRY_PROCESSED_SYNC: Once<Barrier> = Once::new();

    trace!("Beginning multiprocessing synchronization / bootloader memory reclaim procedure.");

    // If this this the bootstrap processor context, the requests will have been
    // passed.
    if let Some((mp_request, memory_map_request)) = bsp_requests {
        // Begin multiprocessing and store the processor count to use in synchronization
        // later.
        if let Some(hwthread_count) = crate::cpu::begin_multiprocessing(mp_request) {
            trace!("We will synchronize {hwthread_count} hardware threads.");

            ENTRY_READY_SYNC.call_once(|| Barrier::new(hwthread_count));
            ENTRY_PROCESSED_SYNC.call_once(|| Barrier::new(hwthread_count));
        }

        debug!("Begin reclaiming bootloader memory...");

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

                trace!("Attempting to free memory: {entry_start:#X}:{entry_end:#X}");

                entry_start..entry_end
            })
            .filter(|entry_range| {
                // Check if the entry contains the BSP stack, and if so, filter it
                // (check returned false, so invert and return true to avoid filtering).
                !check_range_contains_stack(entry_range, &stack_range)
            })
            .filter(|entry_range| {
                // If the synchronizer hasn't been initialized, then multiprocessing was
                // disabled, and no extra entry checks need to occur.
                let (Some(entry_ready), Some(entry_processed)) =
                    (ENTRY_READY_SYNC.get(), ENTRY_PROCESSED_SYNC.get())
                else {
                    return true;
                };

                // Set the new entry to be checked.
                let mut entry_check = ENTRY_CHECK.write();
                *entry_check = Some((entry_range.clone(), Atomic::<bool>::new(false)));
                drop(entry_check);

                trace!("Waiting for all hardware threads to be ready for next entry...");
                entry_ready.wait();

                trace!("Waiting for all hardware threads to check entry...");
                entry_processed.wait();

                let entry_check = ENTRY_CHECK.read();
                let (_, is_entry_used) = entry_check.as_ref().expect("entry check is empty");

                is_entry_used.load(Ordering::Relaxed)
            })
            // We'll flatten each entry to a physical memory range...
            .flatten()
            // Iterate page-size chunks...
            .step_by(page_size())
            // Map entry to physical page address...
            .map(|address| Address::<Frame>::new(address).unwrap())
            // Free the requisite physical frames...
            .try_for_each(PhysicalMemoryManager::free_frame)
            .expect("could not free bootloader reclaimable memory frame");

        if let Some(entry_ready) = ENTRY_READY_SYNC.get() {
            // Clear the check entry to `None`, so other hardware threads know there's no
            // more work.
            let mut entry_check = ENTRY_CHECK.write();
            *entry_check = None;
            drop(entry_check);

            // Signal to other hardware threads to read the next extry.
            entry_ready.wait();
        }

        debug!("Bootloader memory reclaimed.");
    } else {
        // Wait for bootstrap processor to populate the synchronizer...
        let entry_ready = ENTRY_READY_SYNC.wait();
        let entry_processed = ENTRY_PROCESSED_SYNC.wait();

        trace!("Entering memory map entry stack check loop.");

        loop {
            trace!("Waiting for next entry to be ready...");
            entry_ready.wait();

            trace!("Waiting to acquire entry...");
            let entry_check = ENTRY_CHECK.read();

            let Some((entry_range, is_entry_used)) = entry_check.as_ref() else {
                // If the entry is `None`, then we're done checking entries.
                break;
            };

            if check_range_contains_stack(entry_range, &stack_range) {
                is_entry_used.store(true, Ordering::Relaxed);
            }

            // Return the entry for other hardware threads to check.
            drop(entry_check);

            trace!("Waiting for entry to finish being checked...");
            entry_processed.wait();
        }

        trace!("Entry checks complete.");
    }

    debug!("Preparing hardware thread for task scheduling...");

    #[cfg(target_arch = "x86_64")]
    crate::arch::x86_64::structures::tss::TaskStateSegment::load_local();

    LocalState::init();

    core::arch::breakpoint();

    // Ensure we enable interrupts prior to enabling the scheduler.
    crate::interrupts::enable();

    // // Safety: The hardware thread is ready to be scheduled with tasks.
    // unsafe {
    //     crate::cpu::local_state::begin_scheduling();
    // }

    // This interrupt wait loop is necessary to ensure the core can jump into the
    // scheduler.
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

/// Murder—in cold electrons—the current hardware thread.
#[inline(never)]
pub fn halt_and_catch_fire() -> ! {
    crate::interrupts::disable();

    crate::interrupts::wait_indefinite()
}
