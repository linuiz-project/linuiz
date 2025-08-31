#[cfg(feature = "panic_traces")]
pub mod tracing;

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // This function should NEVER panic or abort.

    error!(
        "KERNEL PANIC ({}): {}",
        info.location().unwrap_or(core::panic::Location::caller()),
        info.message()
    );

    #[cfg(feature = "panic_traces")]
    tracing::emit_stack_trace();

    crate::cpu::halt_and_catch_fire()
}
