pub static KERNEL_SYMBOLS: spin::Once<&[(&str, elf::symbol::Symbol)]> = spin::Once::new();

const MAXIMUM_STACK_TRACE_DEPTH: usize = 16;

/// Traces the frame pointer, storing the traced return addresses within the provided array. Returns whether the trace overflowed the array.
fn trace_frame_pointer(
    stack_trace_addresses: &mut spin::MutexGuard<'static, [Option<u64>; MAXIMUM_STACK_TRACE_DEPTH]>,
) -> bool {
    // REMARK: This function should *never* panic or abort.

    #[repr(C)]
    #[derive(Debug)]
    struct StackFrame {
        prev_frame_ptr: *const StackFrame,
        return_address: u64,
    }

    let mut stack_trace_index: u8 = 0;
    let mut frame_ptr: *const StackFrame;
    // Safety: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };
    // Safety: Stack frame pointer should be valid, if `rbp` is being used correctly.
    // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
    while let Some(stack_frame) = unsafe { frame_ptr.as_ref() } {
        // 'Push' the return address to the array.
        let Some(stack_trace_address) = stack_trace_addresses.get_mut(stack_trace_index as usize) else { return true };

        if stack_frame.return_address == 0x0 {
            break;
        }

        *stack_trace_address = Some(stack_frame.return_address);

        frame_ptr = stack_frame.prev_frame_ptr;
        stack_trace_index += 1;
    }

    false
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // REMARK: This function should *never* panic or abort.

    use core::sync::atomic::{AtomicBool, Ordering};

    static STACK_TRACE_IN_PROGRESS: AtomicBool = AtomicBool::new(false);
    static STACK_TRACE_ADDRESSES: spin::Mutex<[Option<u64>; MAXIMUM_STACK_TRACE_DEPTH]> =
        spin::Mutex::new([None; MAXIMUM_STACK_TRACE_DEPTH]);

    while STACK_TRACE_IN_PROGRESS.compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed).is_err() {
        core::hint::spin_loop();
    }

    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap_or(core::panic::Location::caller()),
        info.message().unwrap_or(&format_args!("no panic message"))
    );

    if let Some(symbols) = KERNEL_SYMBOLS.get() {
        let (stack_traces, trace_overflow) = {
            let mut stack_trace_addresses = STACK_TRACE_ADDRESSES.lock();
            // Stack trace addresses may have been set by other panicks.
            stack_trace_addresses.fill(None);

            (*stack_trace_addresses, trace_frame_pointer(&mut stack_trace_addresses))
        };

        error!("----------STACK-TRACE---------");

        /// Pretty-prints a stack-trace entry.
        fn print_stack_trace_entry<D: core::fmt::Display>(entry_num: usize, fn_address: u64, symbol_name: Option<D>) {
            let tab_len = 4 + (entry_num * 2);

            if let Some(symbol_name) = symbol_name {
                error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {symbol_name:#}");
            } else {
                error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} !!! no function found !!!");
            }
        }

        let mut trace_index = 0;

        for fn_address in stack_traces.iter().rev().filter_map(|fn_address| *fn_address) {
            const SYMBOL_TYPE_FUNCTION: u8 = 2;

            if let Some((symbol_name, symbol)) =
                symbols.iter().filter(|(_, symbol)| symbol.st_symtype() == SYMBOL_TYPE_FUNCTION).find(|(_, symbol)| {
                    let symbol_start = symbol.st_value;
                    let symbol_end = symbol_start + symbol.st_size;

                    (symbol_start..symbol_end).contains(&fn_address)
                })
            {
                match rustc_demangle::try_demangle(symbol_name) {
                    Ok(demangled) => print_stack_trace_entry(trace_index, symbol.st_value, Some(demangled)),
                    Err(_) => print_stack_trace_entry(trace_index, symbol.st_value, Some(symbol_name)),
                }
            } else {
                print_stack_trace_entry::<u8>(trace_index, fn_address, None);
            }

            trace_index += 1;
        }

        if trace_overflow {
            error!("More entries, stack trace overflowed.");
        } else if trace_index == 0 {
            error!("Unable to produce stack trace.");
        }

        error!("----------STACK-TRACE----------");

        STACK_TRACE_IN_PROGRESS.store(false, Ordering::Relaxed);
    }

    // Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}

#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    // Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}
