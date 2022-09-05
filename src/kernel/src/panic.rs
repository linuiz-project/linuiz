pub static DEBUG_TABLES: spin::Once<(&[crate::elf::symbol::Symbol], &[u8])> = spin::Once::new();

const MAXIMUM_STACK_TRACE_DEPTH: usize = 5;

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
    // SAFETY: Does not corrupt any auxiliary state.
    unsafe { core::arch::asm!("mov {}, rbp", out(reg) frame_ptr, options(nostack, nomem, preserves_flags)) };
    // SAFETY: Stack frame pointer should be valid, if `rbp` is being used correctly.
    // TODO add checks somehow to ensure `rbp` is being used to store the stack base.
    while let Some(stack_frame) = unsafe { frame_ptr.as_ref() } {
        // 'Push' the return address to the array.
        if let Some(stack_trace_address) = stack_trace_addresses.get_mut(stack_trace_index as usize) {
            *stack_trace_address = Some(stack_frame.return_address);
        } else {
            return true;
        }

        frame_ptr = stack_frame.prev_frame_ptr;
        stack_trace_index += 1;
    }

    false
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    // REMARK: This function should *never* panic or abort.

    static STACK_TRACE_ADDRESSES: spin::Mutex<[Option<u64>; MAXIMUM_STACK_TRACE_DEPTH]> =
        spin::Mutex::new([None; MAXIMUM_STACK_TRACE_DEPTH]);

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
    crate::newline!();
    crate::println!("----------STACK-TRACE---------");

    let debug_tables = DEBUG_TABLES.get();
    let mut increment = 0;

    /// Pretty-prints a stack-trace entry.
    fn print_stack_trace_entry(entry_num: usize, fn_address: u64, symbol_name: Option<&str>) {
        let tab_len = 4 + (entry_num * 2);
        crate::println!(
            "{entry_num:.<tab_len$}0x{fn_address:0<16X} {}",
            symbol_name.unwrap_or("!!! no function found !!!")
        );
    }

    for fn_address in stack_traces.iter().rev().filter_map(|fn_address| *fn_address) {
        if let Some((symtab, strtab)) = debug_tables
            && let Some(fn_symbol) = symtab
                .iter()
                .filter(|symbol| symbol.get_type() == crate::elf::symbol::Type::Function)
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
            print_stack_trace_entry(increment, fn_symbol.get_value(), Some(symbol_name));
        } else {
            print_stack_trace_entry(increment, fn_address, None);
        }

        increment += 1;
    }

    if trace_overflow {
        print_stack_trace_entry(increment, 0x0, Some("!!! trace overflowed !!!"))
    }

    crate::println!("----------STACK-TRACE----------");

    crate::interrupts::wait_loop()
}

#[alloc_error_handler]
fn alloc_error(error: core::alloc::Layout) -> ! {
    error!("KERNEL ALLOCATOR PANIC: {:?}", error);

    // TODO should we actually abort on every alloc error?
    crate::interrupts::wait_loop()
}
