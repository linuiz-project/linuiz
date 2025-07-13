#[cfg(feature = "panic_traces")]
pub mod tracing;

/// # Remarks
///
/// This function should *never* panic or abort.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap_or(core::panic::Location::caller()),
        info.message()
    );

    #[cfg(feature = "panic_traces")]
    tracing::emit_stack_trace();

    crate::cpu::halt_and_catch_fire()
}
