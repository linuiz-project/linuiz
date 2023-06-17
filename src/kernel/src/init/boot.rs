use core::sync::atomic::{AtomicBool, Ordering};
use libsys::{Address, Virtual};

mod ignore {
    ///! This module is never exported. It is used for bootloader requests that should never be accessed in software.

    #[limine::limine_tag]
    static LIMINE_STACK: limine::StackSizeRequest = limine::StackSizeRequest::new(super::LIMINE_REV).stack_size({
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

crate::error_impl! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Error {
        BootExpired => None,
        NoKernelFile => None,
        NoRsdpAddress => None,
        NoMemoryMap => None
    }
}

pub const LIMINE_REV: u64 = 0;

static BOOT_RECLAIM: AtomicBool = AtomicBool::new(false);

macro_rules! boot_only {
    ($code:block) => {{
        if !BOOT_RECLAIM.load(Ordering::Acquire) {
            Ok($code)
        } else {
            Err(Error::BootExpired)
        }
    }};
}

pub fn get_memory_map() -> Result<&'static [&'static limine::MemmapEntry]> {
    boot_only!({
        #[limine::limine_tag]
        static LIMINE_MMAP: limine::MemmapRequest = limine::MemmapRequest::new(LIMINE_REV);

        LIMINE_MMAP.get_response().map(limine::MemmapResponse::get_memmap).ok_or(Error::NoMemoryMap)
    })
    .flatten()
}

pub fn get_rsdp_address() -> Result<Address<Virtual>> {
    boot_only!({
        #[limine::limine_tag]
        static LIMINE_RSDP: limine::RsdpRequest = limine::RsdpRequest::new(LIMINE_REV);

        LIMINE_RSDP
            .get_response()
            .and_then(limine::RsdpResponse::address)
            .and_then(|ptr| {
                Address::new(
                    // Properly handle the bootloader's mapping of ACPI addresses in lower-half or higher-half memory space.
                    core::cmp::min(ptr.addr().get(), ptr.addr().get().wrapping_sub(crate::mem::HHDM.address().get())),
                )
            })
            .ok_or(Error::NoRsdpAddress)
    })
    .flatten()
}

pub fn kernel_file() -> Result<&'static limine::File> {
    boot_only!({
        #[limine::limine_tag]
        static LIMINE_KERNEL_FILE: limine::KernelFileRequest = limine::KernelFileRequest::new(LIMINE_REV);

        LIMINE_KERNEL_FILE.get_response().map(limine::KernelFileResponse::file).ok_or(Error::NoKernelFile)
    })
    .flatten()
}

/// # Safety
///
/// No dangling references can remain to bootloader types or memory, as it may be concurrently overwritten.
pub unsafe fn reclaim_boot_memory(skip_ranges: &[core::ops::Range<usize>]) {
    use crate::mem::alloc::pmm::{FrameType, PMM};

    assert!(!BOOT_RECLAIM.load(Ordering::Acquire));

    // TODO
    // for frame in get_memory_map()
    //     .unwrap()
    //     .iter()
    //     .filter(|entry| entry.ty() == limine::MemoryMapEntryType::BootloaderReclaimable)
    //     .flat_map(|entry| entry.range().step_by(page_size()))
    //     .map(|address| Address::<libsys::Frame>::new_truncate(address.try_into().unwrap()))
    //     .filter(|address| skip_ranges.iter().any(|skip| skip.contains(&address.get().get())))
    // {
    //     PMM.modify_type(frame, FrameType::Generic, Some(FrameType::BootReclaim)).ok();
    // }

    // BOOT_RECLAIM.store(true, Ordering::Release);
}
