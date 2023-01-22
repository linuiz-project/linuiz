use spin::Once;

pub static KERNEL_SYMBOLS: Once<&'static [libkernel::elf::symbol::Symbol]> = Once::new();
pub static KERNEL_STRINGS: Once<&'static [u8]> = Once::new();

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
    // ### Safety: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };
    // ### Safety: Stack frame pointer should be valid, if `rbp` is being used correctly.
    // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
    while let Some(stack_frame) = unsafe { frame_ptr.as_ref() } {
        // 'Push' the return address to the array.
        let Some(stack_trace_address) = stack_trace_addresses.get_mut(stack_trace_index as usize) else { return true };

        if stack_frame.return_address == 0x0 {
            break;
        } else {
            *stack_trace_address = Some(stack_frame.return_address);

            frame_ptr = stack_frame.prev_frame_ptr;
            stack_trace_index += 1;
        }
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

    let (stack_traces, trace_overflow) = {
        let mut stack_trace_addresses = STACK_TRACE_ADDRESSES.lock();
        let trace_overflow = trace_frame_pointer(&mut stack_trace_addresses);
        let stack_trace_addresses_clone = stack_trace_addresses.clone();
        // Ensure we reset the stack trace addresses for other panicks.
        stack_trace_addresses.fill(None);

        (stack_trace_addresses_clone, trace_overflow)
    };

    error!("----------STACK-TRACE---------");

    enum SymbolName<'a> {
        Demangled(&'a rustc_demangle::Demangle<'a>),
        RawStr(&'a str),
        None,
    }

    /// Pretty-prints a stack-trace entry.
    fn print_stack_trace_entry(entry_num: usize, fn_address: u64, symbol_name: SymbolName) {
        let tab_len = 4 + (entry_num * 2);

        match symbol_name {
            SymbolName::Demangled(demangled) => {
                error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {demangled:#}")
            }
            SymbolName::RawStr(raw_str) => error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {raw_str}"),
            SymbolName::None => error!("{entry_num:.<tab_len$}0x{fn_address:0<16X} !!! no function found !!!"),
        }
    }

    let mut trace_index = 0;

    if let Some(symtab) = KERNEL_SYMBOLS.get() && let Some(strtab) = KERNEL_STRINGS.get() {
        for fn_address in stack_traces.iter().rev().filter_map(|fn_address| *fn_address) {
            if let Some(fn_symbol) = symtab
                    .iter()
                    .filter(|symbol| symbol.get_type() == libkernel::elf::symbol::Type::Function)
                    .find(|symbol| {
                        let symbol_start = symbol.get_value();
                        let symbol_end = symbol_start + (symbol.get_size() as u64);

                        (symbol_start..symbol_end).contains(&fn_address)
                    })
                && let Some(fn_name_offset) = fn_symbol.get_name_offset()
                && let Some(symbol_name) = core::ffi::CStr::from_bytes_until_nul(&strtab[fn_name_offset..])
                    .ok()
                    .and_then(|cstr| cstr.to_str().ok())
            {
                match rustc_demangle::try_demangle(symbol_name) {
                    Ok(demangled) => {
                        print_stack_trace_entry(
                            trace_index,
                            fn_symbol.get_value(),
                            SymbolName::Demangled(&demangled)
                        )
                    },

                    Err(_) => {
                        print_stack_trace_entry(
                            trace_index,
                            fn_symbol.get_value(),
                            SymbolName::RawStr(symbol_name)
                        )
                    }
                }
            } else {
                print_stack_trace_entry(trace_index, fn_address, SymbolName::None);
            }

            trace_index += 1;
        }
    }

    if trace_overflow {
        print_stack_trace_entry(trace_index, 0x0, SymbolName::RawStr("!!! trace overflowed !!!"))
    } else if trace_index == 0 {
        error!("No stack trace is available.");
    }

    error!("----------STACK-TRACE----------");

    STACK_TRACE_IN_PROGRESS.store(false, Ordering::Relaxed);

    // ### Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}

#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    // ### Safety: It's dead, Jim.
    unsafe { crate::interrupts::halt_and_catch_fire() }
}
