#[cfg(feature = "panic_traces")]
pub mod tracing;

#[cfg(not(test))]
#[panic_handler]
fn panic(panic_info: &core::panic::PanicInfo) -> ! {
    // This function should NEVER panic or abort.

    let panic_location = panic_info
        .location()
        .unwrap_or(core::panic::Location::caller());
    let panic_message = panic_info.message();
    let default_panic_message = format_args!("KERNEL PANIC ({panic_location}): {panic_message}");

    cfg_select! {
        feature = "panic_traces" => {
            use crate::util::sync::Mutex;

            type PanicStringBuffer = heapless::String<0x4000>;
            static PANIC_STRING_BUFFER: Mutex<PanicStringBuffer> = Mutex::new(PanicStringBuffer::new());

            PANIC_STRING_BUFFER.with_lock(|panic_string_buffer| {
                panic_string_buffer.clear();

                if let Err(error) = tracing::construct_panic_message(&mut *panic_string_buffer) {
                    error!("{default_panic_message}\n\tFailed constructing panic message: {error:?}");
                } else {
                    error!("{default_panic_message}\n{panic_string_buffer}");
                }
            });
        }

        not(feature = "panic_traces") => {
            error!("{default_panic_message}");
        }
    }

    crate::cpu::halt_and_catch_fire()
}
