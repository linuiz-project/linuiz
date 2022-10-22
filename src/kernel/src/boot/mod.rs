use core::sync::atomic::{AtomicBool, Ordering};

mod ignore {
    ///! This module is never exported. It is used for bootloader requests that should never be accessed in software.

    static LIMINE_STACK: limine::LimineStackSizeRequest = limine::LimineStackSizeRequest::new(super::LIMINE_REV)
        .stack_size({
            #[cfg(debug_assertions)]
            {
                0x1000000
            }

            #[cfg(not(debug_assertions))]
            {
                0x4000
            }
        });
}

pub const LIMINE_REV: u64 = 0;

static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(LIMINE_REV);
static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(LIMINE_REV);
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);
static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(LIMINE_REV);

static BOOT_RECLAIM: AtomicBool = AtomicBool::new(false);

macro_rules! boot_only {
    ($code:block) => {{
        if BOOT_RECLAIM.load(Ordering::Relaxed) {
            None
        } else {
            $code
        }
    }};
}

pub fn get_memory_map() -> Option<&'static [limine::NonNullPtr<limine::LimineMemmapEntry>]> {
    boot_only!({ LIMINE_MMAP.get_response().get().map(|response| response.memmap()) })
}

pub fn get_kernel_file() -> Option<&'static limine::LimineFile> {
    boot_only!({ LIMINE_KERNEL_FILE.get_response().get().and_then(|response| response.kernel_file.get()) })
}

pub fn get_rsdp_address() -> Option<libcommon::Address<libcommon::Physical>> {
    boot_only!({
        LIMINE_RSDP.get_response().get().and_then(|response| response.address.as_ptr()).and_then(|ptr| {
            libcommon::Address::<libcommon::Physical>::new(
                // Properly handle the bootloader's mapping of ACPI addresses in lower-half or higher-half memory space.
                core::cmp::min(ptr.addr(), ptr.addr().wrapping_sub(crate::memory::get_hhdm_address().as_usize()))
                    as u64,
            )
        })
    })
}
