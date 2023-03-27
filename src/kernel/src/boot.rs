use core::sync::atomic::{AtomicBool, Ordering};
use libsys::{Address, Virtual};

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
    boot_only!({
        static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(LIMINE_REV);
        LIMINE_MMAP.get_response().get().map(|response| response.memmap())
    })
}

pub fn get_kernel_modules() -> Option<&'static [limine::NonNullPtr<limine::LimineFile>]> {
    boot_only!({
        static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);
        LIMINE_MODULES.get_response().get().map(limine::LimineModuleResponse::modules)
    })
}

pub fn get_rsdp_address() -> Option<Address<Virtual>> {
    boot_only!({
        static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(LIMINE_REV);
        LIMINE_RSDP.get_response().get().and_then(|response| response.address.as_ptr()).and_then(|ptr| {
            Address::new(
                // Properly handle the bootloader's mapping of ACPI addresses in lower-half or higher-half memory space.
                core::cmp::min(ptr.addr(), ptr.addr().wrapping_sub(crate::memory::hhdm_address().get())),
            )
        })
    })
}

/// # Safety
///
/// No dangling references can remain to bootloader types or memory, as it may be concurrently overwritten.
pub unsafe fn reclaim_boot_memory(skip_ranges: &[core::ops::Range<usize>]) {
    use crate::memory::pmm::FrameType;
    use limine::LimineMemoryMapEntryType;

    assert!(!BOOT_RECLAIM.load(Ordering::Acquire));

    for frame in get_memory_map()
        .unwrap()
        .iter()
        .filter(|entry| entry.typ == LimineMemoryMapEntryType::BootloaderReclaimable)
        .flat_map(|entry| (entry.base..(entry.base + entry.len)).step_by(0x1000))
        .map(|address| Address::<libsys::Frame>::new_truncate(address as usize))
        .filter(|address| skip_ranges.iter().any(|skip| skip.contains(&address.get().get())))
    {
        crate::memory::PMM.modify_type(frame, FrameType::Generic, Some(FrameType::BootReclaim)).ok();
    }

    BOOT_RECLAIM.store(true, Ordering::Release);
}
