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

static BOOT_RECLAIM: AtomicBool = AtomicBool::new(false);

macro_rules! boot_only {
    ($code:block) => {{
        if BOOT_RECLAIM.load(Ordering::Acquire) {
            None
        } else {
            $code
        }
    }};
}

pub fn get_memory_map() -> Option<&'static [limine::NonNullPtr<limine::LimineMemmapEntry>]> {
    static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(LIMINE_REV);
    boot_only!({ LIMINE_MMAP.get_response().get().map(|response| response.memmap()) })
}

pub fn get_kernel_file() -> Option<&'static limine::LimineFile> {
    static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(LIMINE_REV);
    boot_only!({ LIMINE_KERNEL_FILE.get_response().get().and_then(|response| response.kernel_file.get()) })
}

pub fn get_kernel_modules() -> Option<&'static [limine::NonNullPtr<limine::LimineFile>]> {
    static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);
    boot_only!({ LIMINE_MODULES.get_response().get().map(|response| response.modules()) })
}

pub fn get_rsdp_address() -> Option<libcommon::Address<libcommon::Physical>> {
    static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(LIMINE_REV);
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

/// # Safety
///
/// No dangling references can remain to bootloader types or memory, as it may be concurrently overwritten.
pub unsafe fn reclaim_boot_memory() {
    use crate::memory::pmm::FrameType;
    use libcommon::{Address, Frame};
    use limine::LimineMemoryMapEntryType;

    assert!(!BOOT_RECLAIM.load(Ordering::Acquire));

    for frame in get_memory_map()
        .unwrap()
        .iter()
        .filter(|entry| entry.typ == LimineMemoryMapEntryType::BootloaderReclaimable)
        .flat_map(|entry| (entry.base..(entry.base + entry.len)).step_by(0x1000))
        .map(|address| Address::<Frame>::from_u64_truncate(address))
    {
        crate::memory::PMM.modify_type(frame, FrameType::Generic, Some(FrameType::BootReclaim)).ok();
    }

    BOOT_RECLAIM.store(true, Ordering::Release);
}
