use crate::memory::get_hhdm_address;
use libarch::interrupts::ControlFlowContext;
use libcommon::{Address, Page};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syscall {
    /// Logs to the kernel standard output.
    ///
    /// Vector: 0x100
    Log { level: log::Level, cstr_ptr: *const core::ffi::c_char },
}

#[no_mangle]
#[repr(align(0x10))]
fn __syscall_handler(
    vector: u64,
    arg0: u64,
    arg1: u64,
    _arg2: u64,
    _arg3: u64,
    _arg4: u64,
    ret_ip: u64,
    ret_sp: u64,
    _regs: &mut libarch::interrupts::SyscallContext,
) -> ControlFlowContext {
    let syscall = match vector {
        0x100 => {
            use log::Level;

            // TODO possibly PR the `log` crate to make `log::Level::from_usize()` public.
            let log_level = match arg0 {
                1 => Ok(Level::Error),
                2 => Ok(Level::Warn),
                3 => Ok(Level::Info),
                4 => Ok(Level::Debug),
                arg0 => Err(arg0),
            };

            match log_level {
                Ok(level) => Some(Syscall::Log { level, cstr_ptr: arg1 as usize as *const _ }),
                Err(invalid_level) => {
                    warn!("Invalid log level provided: {}", invalid_level);
                    None
                }
            }
        }

        vector => {
            warn!("Unhandled system call vector: {:#X}", vector);
            None
        }
    };

    match syscall {
        Some(syscall) => do_syscall(syscall),
        None => warn!("Failed to execute system call."),
    }

    ControlFlowContext { ip: ret_ip, sp: ret_sp }
}

pub fn do_syscall(vector: Syscall) {
    match vector {
        Syscall::Log { level, cstr_ptr } => {
            // SAFETY: The kernel guarantees the HHDM will be valid.
            let page_manager = unsafe { crate::memory::Mapper::from_current(get_hhdm_address()) };

            let mut cstr_increment_ptr = cstr_ptr;
            let mut last_char_page_base = Address::<Page>::new(Address::zero(), None).unwrap();
            loop {
                // Ensure the memory of the current cstr increment address is mapped.
                let Some(char_address_base_page) = Address::<Page>::from_ptr(cstr_increment_ptr, None)
                    else {
                        warn!("Process attempted to overrun with `CStr` pointer.");
                        return;
                    };
                if char_address_base_page.index() > last_char_page_base.index() {
                    if page_manager.is_mapped(char_address_base_page) {
                        last_char_page_base = char_address_base_page;
                    } else {
                        // TODO do something more comprehensive here
                        warn!("Process attempted to log with unmapped `CStr` memory.");
                        return;
                    }
                }

                // SAFETY: Pointer is proven-mapped, is a numeric primitive (so cannot be 'uninitialized' in this context).
                if unsafe { cstr_increment_ptr.read_unaligned() } == 0 {
                    break;
                } else {
                    match (cstr_increment_ptr as isize).checked_add(1) {
                        Some(new_ptr) => cstr_increment_ptr = new_ptr as *const _,
                        None => {
                            warn!("Process attempted to overflow with `CStr` pointer.");
                            return;
                        }
                    }
                }
            }

            // SAFETY: At this point, the `CStr` pointer should be completely known-valid.
            match unsafe { core::ffi::CStr::from_ptr(cstr_ptr) }.to_str() {
                Ok(string) => log!(level, "{}", string),
                Err(error) => {
                    warn!("Process provided invalid `CStr` for logging: {:?}", error);
                    return;
                }
            }
        }
    }
}
