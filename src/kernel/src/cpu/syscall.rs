use crate::memory::get_hhdm_address;
use lzstd::{Address, Page};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syscall {
    /// Logs to the kernel standard output.
    ///
    /// Vector: 0x100
    Log { level: log::Level, cstr_ptr: *const core::ffi::c_char },
}

pub fn do_syscall(vector: Syscall) {
    match vector {
        Syscall::Log { level, cstr_ptr } => {
            // ### Safety: The kernel guarantees the HHDM will be valid.
            let page_manager = unsafe { crate::memory::address_space::Mapper::from_current(get_hhdm_address()) };

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
                        warn!("Process attempted to log with unmapped `CStr` memory.");
                        return;
                    }
                }

                // ### Safety: Pointer is proven-mapped, is a numeric primitive (so cannot be 'uninitialized' in this context).
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

            // ### Safety: At this point, the `CStr` pointer should be completely known-valid.
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
