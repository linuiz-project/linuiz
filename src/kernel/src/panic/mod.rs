pub mod symbols;

use core::ptr::NonNull;
use libsys::{Address, Virtual};

#[repr(C)]
#[derive(Debug)]
struct StackFrame {
    prev_frame_ptr: Option<NonNull<StackFrame>>,
    return_address: Address<Virtual>,
}

struct StackTracer {
    frame_ptr: Option<NonNull<StackFrame>>,
}

impl StackTracer {
    /// # Safety
    ///
    /// The provided frame pointer must point to a valid call stack frame.
    const unsafe fn new(frame_ptr: NonNull<StackFrame>) -> Self {
        Self {
            frame_ptr: Some(frame_ptr),
        }
    }
}

impl Iterator for StackTracer {
    type Item = Address<Virtual>;

    fn next(&mut self) -> Option<Self::Item> {
        // Safety: Stack frame pointer will be valid if the correct value is provided to `Self::new()`.
        let stack_frame = unsafe { self.frame_ptr?.as_ref() };
        self.frame_ptr = stack_frame.prev_frame_ptr;

        Some(stack_frame.return_address)
    }
}

/// # Remark
///
/// This function should *never* panic or abort.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap_or(core::panic::Location::caller()),
        info.message()
    );

    stack_trace();

    crate::interrupts::halt_and_catch_fire()
}

fn stack_trace() {
    fn print_stack_trace_entry<D: core::fmt::Display>(
        entry_num: usize,
        fn_address: Address<Virtual>,
        symbol_name: D,
    ) {
        error!("#{entry_num: <4}0x{:X} {symbol_name:#}", fn_address.get());
    }

    error!("----------STACK-TRACE---------");

    let frame_ptr = {
        #[cfg(target_arch = "x86_64")]
        {
            let base_ptr: usize;

            // Safety: We're just reading a register.
            unsafe {
                core::arch::asm!(
                    "mov {}, rbp",
                    out(reg) base_ptr,
                    options(nostack, nomem, preserves_flags)
                );
            }

            core::ptr::without_provenance::<StackFrame>(base_ptr)
        }
    };

    let Some(frame_ptr) = NonNull::new(frame_ptr.cast_mut()) else {
        error!("No stack frame pointer was found; stack trace will not be emitted.");
        return;
    };

    // Safety: Frame pointer is pulled directly from the frame pointer register.
    let stack_tracer = unsafe { StackTracer::new(frame_ptr) };

    for (depth, trace_address) in stack_tracer.enumerate() {
        const SYMBOL_TYPE_FUNCTION: u8 = 2;

        if let Some(symbol_name) = symbols::get_name(trace_address) {
            if let Ok(demangled) = rustc_demangle::try_demangle(symbol_name) {
                print_stack_trace_entry(depth, trace_address, demangled);
            } else {
                print_stack_trace_entry(depth, trace_address, symbol_name);
            }
        } else {
            print_stack_trace_entry(depth, trace_address, "!!! no function found !!!");
        }
    }

    error!("----------STACK-TRACE----------");
}
