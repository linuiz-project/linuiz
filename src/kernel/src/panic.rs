use elf::{
    endian::AnyEndian,
    string_table::StringTable,
    symbol::{Symbol, SymbolTable},
};
use libsys::{Address, Virtual};

pub static KERNEL_SYMBOLS: spin::Once<(SymbolTable<'static, AnyEndian>, StringTable<'static>)> = spin::Once::new();

fn get_symbol(address: Address<Virtual>) -> Option<(Option<&'static str>, Symbol)> {
    KERNEL_SYMBOLS.get().and_then(|(symbols, strings)| {
        let symbol = symbols.iter().find(|symbol| {
            let symbol_region = symbol.st_value..(symbol.st_value + symbol.st_size);
            symbol_region.contains(&address.get().try_into().unwrap())
        })?;
        let symbol_name = strings.get(symbol.st_name as usize).ok();

        Some((symbol_name, symbol))
    })
}

const MAXIMUM_STACK_TRACE_DEPTH: usize = 16;

/// Traces the frame pointer, returning an array with the function addresses.
///
/// #### Remark
///
/// This function should *never* panic or abort.
fn trace_frame_pointers() -> [Address<Virtual>; MAXIMUM_STACK_TRACE_DEPTH] {
    #[repr(C)]
    #[derive(Debug)]
    struct StackFrame {
        prev_frame_ptr: *const StackFrame,
        return_address: u64,
    }

    let mut stack_trace_addresses = [{ Address::new_truncate(0) }; MAXIMUM_STACK_TRACE_DEPTH];
    let mut frame_ptr: *const StackFrame;
    // Safety: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };
    for stack_trace_address in &mut stack_trace_addresses {
        // Safety: Stack frame pointer should be valid, if `rbp` is being used correctly.
        // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
        let Some(stack_frame) = (unsafe { frame_ptr.as_ref() }) else { break };

        *stack_trace_address = Address::new_truncate(stack_frame.return_address.try_into().unwrap());
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

    let stack_traces = trace_frame_pointers();
    trace!("Raw stack traces: {:#X?}", stack_traces);
    let trace_len = stack_traces.iter().position(|e| e.get() == 0).unwrap_or(stack_traces.len());
    print_stack_trace(&stack_traces[..trace_len]);

    // Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}

fn print_stack_trace(stack_traces: &[Address<Virtual>]) {
    fn print_stack_trace_entry<D: core::fmt::Display>(
        entry_num: usize,
        fn_address: Address<Virtual>,
        symbol_name: Option<D>,
    ) {
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

            if let Some((Some(symbol_name), _)) = get_symbol(*fn_address) {
                if let Ok(demangled) = rustc_demangle::try_demangle(symbol_name) {
                    print_stack_trace_entry(trace_index, *fn_address, Some(demangled));
                } else {
                    print_stack_trace_entry(trace_index, *fn_address, Some(symbol_name));
                }
            } else {
                print_stack_trace_entry::<&str>(trace_index, *fn_address, None);
            }
        })
        .count();

    if total_traces == 0 {
        error!("Unable to produce stack trace.");
    }

    error!("----------STACK-TRACE----------");
}
