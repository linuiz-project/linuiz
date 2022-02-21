mod int_ctrl;

pub use int_ctrl::*;

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::{arch::asm, num::NonZeroUsize};
use libkernel::registers::msr::{Generic, IA32_KERNEL_GS_BASE};
use spin::{Mutex, MutexGuard};

#[repr(usize)]
pub enum Offset {
    SelfPtr = 0x0,
    ID = 0x10,
    Clock = 0x20,
    IntCtrlPtr = 0x30,
    SchedulerPtr = 0x40,
    SyscallStackPtr = 0x50,
    TrapStackPtr = 0x60,
    DoubleTrapStackPtr = 0x70,
    PriorityTrapStackPtr = 0x80,
    ExceptionStackPtr = 0x90,
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

    let (ptr, size) = libkernel::memory::malloc::get()
        .alloc(
            0x1000,
            // Local state register must be page-aligned.
            NonZeroUsize::new(0x1000),
        )
        .unwrap()
        .into_parts();

    core::ptr::write_bytes(ptr, 0x0, size);

    let alloc_stack = |size| {
        let (ptr, len) = libkernel::memory::malloc::get()
            .alloc(size, NonZeroUsize::new(16))
            .unwrap()
            .into_parts();

        ptr.add(len)
    };

    ptr.cast::<*const u8>().write(ptr);
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

#[macro_export]
macro_rules! rdgsval {
    ($ty:ty, $offset:expr) => {
        {
            let val: $ty;

            core::arch::asm!(
                "mov {}, gs:{}",
                out(reg) val,
                const $offset as usize,
                options(nostack, nomem, preserves_flags)
            );

            val
        }
    };

    ($ty:ty, $offset:expr, $reg_size:ident) => {
        {
            let val: $ty;

            core::arch::asm!(
                concat!("mov {:", stringify!($reg_size), "}, gs:{}"),
                out(reg) val,
                const $offset as usize,
                options(nostack, nomem, preserves_flags)
            );

            val
        }
    };
}

#[macro_export]
macro_rules! wrgsval {
    ($offset:expr, $val:expr) => {
        core::arch::asm!(
            "mov gs:{}, {}",
            const $offset as usize,
            in(reg) $val,
            options(nostack, nomem, preserves_flags)
        );
    };
}

#[macro_export]
macro_rules! gs_ptr {
    ($ptr_ty:ty, $offset:expr) => {
        {
            let ptr: *const $ptr_ty;

            asm!(
                "mov {0}, gs:{1}",
                "add {0}, {2}",
                out(reg) ptr,
                const Offset::SelfPtr as usize,
                const $offset as usize,
                options(nostack, nomem)
            );

            ptr
        }
    };
}

/// Creates the core-local interrupt controller.
pub fn create_int_ctrl() {
    unsafe {
        assert_eq!(
            rdgsval!(*mut InterruptController, Offset::IntCtrlPtr),
            core::ptr::null_mut(),
            "Interrupt controller can only be created once."
        );

        // Create the interrupt controller.
        let int_ctrl_ptr = libkernel::alloc_obj!(InterruptController);
        int_ctrl_ptr.write(InterruptController::create());

        // Write the interrupt controller pointer.
        wrgsval!(Offset::IntCtrlPtr, int_ctrl_ptr);
    }
}

/// Creates the core-local scheduler.
pub fn create_scheduler() {
    unsafe {
        assert_eq!(
            rdgsval!(*mut Mutex<Scheduler>, Offset::IntCtrlPtr),
            core::ptr::null_mut(),
            "Scheduler can only be created once."
        );

        let scheduler_ptr = libkernel::alloc_obj!(Mutex<Scheduler>);
        scheduler_ptr.write(Mutex::new(Scheduler::new()));

        wrgsval!(Offset::SchedulerPtr, scheduler_ptr);
    }
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn id() -> u32 {
    rdgsval!(u32, Offset::ID, e)
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn clock() -> &'static AtomicClock {
    &*(gs_ptr!(AtomicClock, Offset::Clock))
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn int_ctrl() -> Option<&'static InterruptController> {
    rdgsval!(*const InterruptController, Offset::IntCtrlPtr).as_ref()
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    rdgsval!(*const Mutex<Scheduler>, Offset::SchedulerPtr)
        .as_ref()
        .map(|scheduler| scheduler.lock())
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    rdgsval!(*const Mutex<Scheduler>, Offset::SchedulerPtr)
        .as_ref()
        .and_then(|scheduler| scheduler.try_lock())
}
