mod int_ctrl;

pub use int_ctrl::*;

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::num::NonZeroUsize;
use libkernel::registers::msr::{Generic, IA32_KERNEL_GS_BASE};
use spin::{Mutex, MutexGuard};

#[repr(usize)]
pub enum Offset {
    ID = 0x0,
    Clock = 0x10,
    SyscallStackPtr = 0x18,
    TrapStackPtr = 0x20,
    DoubleTrapStackPtr = 0x28,
    PriorityTrapStackPtr = 0x30,
    ExceptionStackPtr = 0x38,
    IntCtrl = 0x200,
    Scheduler = 0x300,
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init() {
    assert_eq!(
        IA32_KERNEL_GS_BASE::read(),
        0,
        "Local state register has already been configured."
    );

    let ptr = libkernel::memory::malloc::get()
        .alloc(
            0x1000,
            // Local state register must be page-aligned.
            NonZeroUsize::new(0x1000),
        )
        .unwrap()
        .into_parts()
        .0;

    let alloc_stack = |size| {
        let (ptr, len) = libkernel::memory::malloc::get()
            .alloc(size, NonZeroUsize::new(16))
            .unwrap()
            .into_parts();

        ptr.add(len)
    };

    ptr.add(Offset::ID as usize)
        .cast::<u32>()
        .write(libkernel::cpu::get_id());
    ptr.add(Offset::Clock as usize)
        .cast::<AtomicClock>()
        .write(AtomicClock::new());
    ptr.add(Offset::SyscallStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(0x1000));
    ptr.add(Offset::TrapStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(0x1000));
    ptr.add(Offset::DoubleTrapStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(0x1000));
    ptr.add(Offset::PriorityTrapStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(0x1000));
    ptr.add(Offset::ExceptionStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(0x1000));

    // Convert ptr to 64 bit representation, and write metadata into low bits.
    IA32_KERNEL_GS_BASE::write(ptr as u64);
}

pub unsafe fn create_int_ctrl() {
    (IA32_KERNEL_GS_BASE::read() as *mut u8)
        .add(Offset::IntCtrl as usize)
        .cast::<InterruptController>()
        .write(InterruptController::create());
}

pub unsafe fn create_scheduler() {
    (IA32_KERNEL_GS_BASE::read() as *mut u8)
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .write(Mutex::new(Scheduler::new()));
}

/// Enables the core-local state structure.
#[inline]
pub unsafe fn enable() {
    // The `enable()` and `disable()` currently do the exact same thing, but
    // there may be some validation done in the future, to ensure proper usage
    // of `swapgs`.
    x86_64::registers::segmentation::GS::swap()
}

/// Disables the core-local state structure.
#[inline]
pub unsafe fn disable() {
    x86_64::registers::segmentation::GS::swap()
}

pub unsafe fn get_ptr() -> *const u8 {
    let self_ptr: *const u8;

    core::arch::asm!(
        "rdgsbase {}",
        out(reg) self_ptr,
        options(pure, nomem)
    );
    info!("LOCAL_STATE PTR {:?}", self_ptr);

    self_ptr
}

pub unsafe fn id() -> u32 {
    get_ptr().add(Offset::ID as usize).cast::<u32>().read()
}

pub unsafe fn clock() -> Option<&'static AtomicClock> {
    get_ptr()
        .add(Offset::Clock as usize)
        .cast::<AtomicClock>()
        .as_ref()
}

pub unsafe fn int_ctrl() -> Option<&'static InterruptController> {
    get_ptr()
        .add(Offset::IntCtrl as usize)
        .cast::<InterruptController>()
        .as_ref()
}

pub unsafe fn lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get_ptr()
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .map(|scheduler| scheduler.lock())
}

pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get_ptr()
        .add(Offset::Scheduler as usize)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .and_then(|scheduler| scheduler.try_lock())
}
