pub mod context;
pub mod local_state;

pub type CoreId = u32;

pub fn get_id() -> CoreId {
    cfg_select! {
        target_arch = "x86_64" => {
            crate::arch::x86_64::get_processor_id()
        }

        _ => { unimplemented!() }
    }
}

/// Murder—in cold electrons—the current processor.
#[inline(never)]
pub fn halt_and_catch_fire() -> ! {
    crate::interrupts::disable();

    crate::interrupts::wait_indefinite()
}

/// # Safety
///
/// This function has the potential to modify state in such a way as to disrupt
/// software execution. It should be run only once per processor at the very
/// beginning of code execution.
pub unsafe fn configure() {
    cfg_select! {
        target_arch = "x86_64" => {
            // Safety: Caller is required to maintain safety invariants.
            unsafe {
                crate::arch::x86_64::configure_processor();
            }
        }

        _ => { unimplemented!() }
    }
}
