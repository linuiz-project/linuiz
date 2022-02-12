mod int_ctrl;

pub use int_ctrl::*;

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::num::NonZeroUsize;
use lib::registers::msr::{Generic, IA32_KERNEL_GS_BASE};
use spin::{Mutex, MutexGuard};

#[repr(usize)]
pub enum Offset {
    ID = 0x0,
    Clock = 0x10,
    IntCtrl = 0x20,
    Scheduler = 0x60,
    SyscallStackPtr = 0x100,
    TrapStackPtr = 0x110,
}

pub unsafe fn init() {
    assert_eq!(
        IA32_KERNEL_GS_BASE::read(),
        0,
        "Local state register has already been configured."
    );

    unsafe {
        let ptr = lib::memory::malloc::get()
            .alloc(
                0x1000,
                // Local state register must be page-aligned.
                NonZeroUsize::new(0x1000),
            )
            .unwrap()
            .into_parts()
            .0;

        ptr.add(Offset::ID as usize)
            .cast::<u32>()
            .write(lib::cpu::get_id());
        ptr.add(Offset::SyscallStackPtr as usize)
            .cast::<*const u8>()
            .write({
                let (ptr, len) = lib::memory::malloc::get()
                    .alloc(0x1000, NonZeroUsize::new(16))
                    .unwrap()
                    .into_parts();

                ptr.add(len)
            });
        ptr.add(Offset::TrapStackPtr as usize)
            .cast::<*const u8>()
            .write({
                let (ptr, len) = lib::memory::malloc::get()
                    .alloc(0x1000, NonZeroUsize::new(16))
                    .unwrap()
                    .into_parts();

                ptr.add(len)
            });

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        IA32_KERNEL_GS_BASE::write(ptr as u64);
    }
}

pub unsafe fn create_int_ctrl() {
    let ptr = IA32_KERNEL_GS_BASE::read() as *mut u8;
    ptr.add(Offset::Clock as usize)
        .cast::<AtomicClock>()
        .write(AtomicClock::new());
    ptr.add(Offset::IntCtrl as usize)
        .cast::<InterruptController>()
        .write(InterruptController::create());
}

pub unsafe fn create_scheduler() {
    (IA32_KERNEL_GS_BASE::read() as *mut u8)
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .write(Mutex::new(Scheduler::new()));
}

/// Enables the LocalState structure.
pub unsafe fn enable() {
    // The `enable()` and `disable()` currently do the exact same thing, but
    // there may be some validation done in the future, to ensure proper usage
    // of `swapgs`.
    x86_64::registers::segmentation::GS::swap()
}

/// Disables the local state structure.
pub unsafe fn disable() {
    x86_64::registers::segmentation::GS::swap()
}

unsafe fn get() -> *const u8 {
    let self_ptr: *const u8;

    core::arch::asm!(
        "mov {}, gs:0x0",
        out(reg) self_ptr,
        options(pure, nomem)
    );

    self_ptr
}

pub unsafe fn id() -> u32 {
    get().add(Offset::ID as usize).cast::<u32>().read()
}

pub unsafe fn clock() -> Option<&'static AtomicClock> {
    get()
        .add(Offset::Clock as usize)
        .cast::<AtomicClock>()
        .as_ref()
}

pub unsafe fn int_ctrl() -> Option<&'static InterruptController> {
    get()
        .add(Offset::IntCtrl as usize)
        .cast::<InterruptController>()
        .as_ref()
}

pub unsafe fn lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get()
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .map(|scheduler| scheduler.lock())
}

pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get()
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .and_then(|scheduler| scheduler.try_lock())
}
