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
        let Some(stack_trace_address) = stack_trace_addresses.get_mut(stack_trace_index as usize) else { return true; };
        *stack_trace_address = Some(stack_frame.return_address);

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
                crate::println!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {demangled}")
            }
            SymbolName::RawStr(raw_str) => crate::println!("{entry_num:.<tab_len$}0x{fn_address:0<16X} {raw_str}"),
            SymbolName::None => crate::println!("{entry_num:.<tab_len$}0x{fn_address:0<16X} !!! no function found !!!"),
        }
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
            match rustc_demangle::try_demangle(symbol_name).ok() {
                Some(demangled) => {
                    print_stack_trace_entry(
                        increment,
                        fn_symbol.get_value(),
                        SymbolName::Demangled(&demangled)
                    )
                },

                None => {
                    print_stack_trace_entry(
                        increment,
                        fn_symbol.get_value(),
                        SymbolName::RawStr(symbol_name)
                    )
                }
            }
        } else {
            print_stack_trace_entry(increment, fn_address, SymbolName::None);
        }

        increment += 1;
    }

    if trace_overflow {
        print_stack_trace_entry(increment, 0x0, SymbolName::RawStr("!!! trace overflowed !!!"))
    } else if increment == 0 {
        crate::println!("No stack trace is available.");
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

mod mangling {
    pub type Result<T> = core::result::Result<T, Error>;

    pub enum Error {
        Parser,
        Malformed,
        NonAscii,
    }

    pub fn demangle(symbol_name: &str, mut buffer: &mut [u8]) -> Result<()> {
        if !symbol_name.is_ascii() {
            return Err(Error::NonAscii);
        }

        static SKIP_TAGS: [&str; 2] = ["Nv", "Nt"];
        static TYPE_ENCODINGS: [(&str, &str); 21] = [
            ("a", "i8"),
            ("b", "bool"),
            ("c", "char"),
            ("d", "f64"),
            ("e", "str"),
            ("f", "f32"),
            ("h", "u8"),
            ("i", "isize"),
            ("j", "usize"),
            ("l", "i32"),
            ("m", "u32"),
            ("n", "i128"),
            ("o", "u128"),
            ("s", "i16"),
            ("t", "u16"),
            ("u", "()"),
            ("v", "..."),
            ("x", "i64"),
            ("y", "u64"),
            ("z", "!"),
            ("p", "_"),
        ];

        fn str_bytes_to_number(bytes: &[u8]) -> Option<usize> {
            core::str::from_utf8(bytes).ok().and_then(|str| usize::from_str_radix(str, 10).ok())
        }

        fn parse_identifier(bytes: &[u8], buffer: &mut [u8]) -> Result<(usize, usize)> {
            let number_len = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
            let Some(run_len) = str_bytes_to_number(&bytes[..number_len]) else { return Err(Error::Parser) };

            for index in 0..run_len {
                buffer[index] = bytes[number_len + index]
            }

            Ok((number_len, run_len))
        }

        // EXAMPLE: _RNvMNtNtNtNtNtCs9TTdqULsK7Z_6kernel4arch3x6410structures3idt10exceptionsNtB2_9Exception24common_exception_handler

        let mut symbol_bytes = symbol_name.as_bytes();

        if symbol_bytes.starts_with("_R".as_bytes()) {
            symbol_bytes = &symbol_bytes[2..];
        } else {
            return Err(Error::Malformed);
        }

        // Skip skippable tags, such as namespace identifiers.
        loop {
            let Ok(tag_str) = core::str::from_utf8(&symbol_bytes[..2]) else { return Err(Error::Parser) };

            if SKIP_TAGS.contains(&tag_str) {
                symbol_bytes = &symbol_bytes[2..];
            } else {
                break;
            }
        }

        loop {
            match symbol_bytes.get(0) {
                Some(byte_code) if byte_code.is_ascii_digit() => {
                    let (byte_run, buffer_run) = parse_identifier(symbol_bytes, buffer)?;
                    symbol_bytes = &symbol_bytes[byte_run..];
                    buffer = &mut buffer[buffer_run..];
                }

                Some(_) => {
                    symbol_bytes = &symbol_bytes[1..];
                }

                None => return Ok(()),
            }
        }
    }
}
