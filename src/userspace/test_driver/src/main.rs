#![no_std]
#![no_main]

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}

#[global_allocator]
static _NONE: FakeAllocator = FakeAllocator;

struct FakeAllocator;
unsafe impl core::alloc::GlobalAlloc for FakeAllocator {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        todo!()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {
        todo!()
    }
}

#[no_mangle]
extern "C" fn _start() -> ! {
    loop {
        libsys::syscall::klog::info("klog syscall test 1").unwrap();
        libsys::syscall::klog::info("klog syscall test 2").unwrap();
        libsys::syscall::task::yield_task().unwrap();
    }
}
