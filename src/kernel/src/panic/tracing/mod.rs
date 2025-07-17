use core::{
    fmt::{Result, Write},
    ptr::NonNull,
};
use heapless::String;
use libsys::{Address, Virtual};
use spin::Mutex;

pub mod symbols;

pub(super) fn emit_stack_trace() {
    static PANIC_BUFFER: Mutex<String<0x4000>> = Mutex::new(String::new());

    let mut panic_buffer = PANIC_BUFFER.lock();

    if let Err(err) = construct_panic_message(&mut *panic_buffer) {
        error!("Failed constructing panic message: {err:?}");
    }
}

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

#[inline(always)]
fn get_stack_frame_ptr() -> *mut StackFrame {
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

        core::ptr::without_provenance_mut::<StackFrame>(base_ptr)
    }
}

fn construct_panic_message(mut buffer: impl Write) -> Result {
    fn print_stack_trace_entry<D: core::fmt::Display>(
        mut buffer: impl Write,
        entry_num: usize,
        fn_address: Address<Virtual>,
        symbol_name: D,
    ) -> Result {
        writeln!(
            buffer,
            "#{entry_num: <4}0x{:X} {symbol_name:#}",
            fn_address.get()
        )
    }

    let Some(frame_ptr) = NonNull::new(get_stack_frame_ptr()) else {
        writeln!(
            &mut buffer,
            "No stack frame pointer was found; stack trace will not be emitted."
        )?;

        return Ok(());
    };

    writeln!(&mut buffer, "----------STACK-TRACE---------")?;

    // Safety: Frame pointer is pulled directly from the frame pointer register.
    (unsafe { StackTracer::new(frame_ptr) })
        .enumerate()
        .try_for_each(|(depth, trace_address)| {
            const SYMBOL_TYPE_FUNCTION: u8 = 2;

            if symbols::Symbols::is_initialized()
                && let Some(symbol_name) = symbols::Symbols::get_name(trace_address)
            {
                if let Ok(demangled) = rustc_demangle::try_demangle(symbol_name) {
                    print_stack_trace_entry(&mut buffer, depth, trace_address, demangled)
                } else {
                    print_stack_trace_entry(&mut buffer, depth, trace_address, symbol_name)
                }
            } else {
                print_stack_trace_entry(
                    &mut buffer,
                    depth,
                    trace_address,
                    "!!! no function found !!!",
                )
            }
        })?;

    writeln!(&mut buffer, "----------STACK-TRACE----------")?;

    Ok(())
}
