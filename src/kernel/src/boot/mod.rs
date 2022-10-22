use core::sync::atomic::{AtomicBool, Ordering};

mod ignore {
    static LIMINE_STACK: limine::LimineStackSizeRequest = limine::LimineStackSizeRequest::new(LIMINE_REV).stack_size({
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

static LIMINE_MMAP: limine::LimineMemmapRequest = limine::LimineMemmapRequest::new(crate::LIMINE_REV);
static LIMINE_KERNEL_FILE: limine::LimineKernelFileRequest = limine::LimineKernelFileRequest::new(0);
static LIMINE_MODULES: limine::LimineModuleRequest = limine::LimineModuleRequest::new(LIMINE_REV);

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
