use elf::symbol::Symbol;

pub static KERNEL_SYMBOLS: spin::Once<&[(&str, elf::symbol::Symbol)]> = spin::Once::new();

const MAXIMUM_STACK_TRACE_DEPTH: usize = 16;

/// Traces the frame pointer, returning an array with the function addresses.
///
/// #### Remark
///
/// This function should *never* panic or abort.
fn trace_frame_pointers() -> [u64; MAXIMUM_STACK_TRACE_DEPTH] {
    #[repr(C)]
    #[derive(Debug)]
    struct StackFrame {
        prev_frame_ptr: *const StackFrame,
        return_address: u64,
    }

    let mut stack_trace_addresses = [0u64; MAXIMUM_STACK_TRACE_DEPTH];
    let mut frame_ptr: *const StackFrame;
    // Safety: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };
    for stack_trace_address in stack_trace_addresses.iter_mut() {
        // Safety: Stack frame pointer should be valid, if `rbp` is being used correctly.
        // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
        let Some(stack_frame) = (unsafe { frame_ptr.as_ref() }) else { break };

        *stack_trace_address = stack_frame.return_address;
        frame_ptr = stack_frame.prev_frame_ptr;
    }

    stack_trace_addresses
}

/// #### Remark
///
/// This function should *never* panic or abort.
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    error!(
        "KERNEL PANIC (at {}): {}",
        info.location().unwrap_or(core::panic::Location::caller()),
        info.message().unwrap_or(&format_args!("no panic message"))
    );

    // TODO use unwinding crate to get a backtrace
    // if let Some(symbols) = KERNEL_SYMBOLS.get() {
    //     let stack_traces = trace_frame_pointers();
    //     trace!("Raw stack traces: {:#X?}", stack_traces);

    //     let trace_len = stack_traces.iter().position(|e| *e == 0).unwrap_or(stack_traces.len());
    //     print_stack_trace(symbols, &stack_traces[..trace_len]);
    // }

    // Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}

fn print_stack_trace(symbols: &[(&str, Symbol)], stack_traces: &[u64]) {
    fn print_stack_trace_entry<D: core::fmt::Display>(entry_num: usize, fn_address: u64, symbol_name: Option<D>) {
        let tab_len = 4 + (entry_num * 2);

        if let Some(symbol_name) = symbol_name {
            error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {symbol_name:#}");
        } else {
            error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} !!! no function found !!!");
        }
    }

    error!("----------STACK-TRACE---------");

    let total_traces = stack_traces
        .iter()
        .rev()
        .enumerate()
        .map(|(trace_index, fn_address)| {
            const SYMBOL_TYPE_FUNCTION: u8 = 2;

            let symbol =
                symbols.iter().filter(|(_, symbol)| symbol.st_symtype() == SYMBOL_TYPE_FUNCTION).find(|(_, symbol)| {
                    let symbol_start = symbol.st_value;
                    let symbol_end = symbol_start + symbol.st_size;

                    (symbol_start..symbol_end).contains(&fn_address)
                });

            if let Some((symbol_name, symbol)) = symbol {
                match rustc_demangle::try_demangle(symbol_name) {
                    Ok(demangled) => print_stack_trace_entry(trace_index, symbol.st_value, Some(demangled)),
                    Err(_) => print_stack_trace_entry(trace_index, symbol.st_value, Some(symbol_name)),
                }
            } else {
                print_stack_trace_entry::<u8>(trace_index, *fn_address, None);
            }
        })
        .count();

    if total_traces == 0 {
        error!("Unable to produce stack trace.");
    }

    error!("----------STACK-TRACE----------");
}
