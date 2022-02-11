mod int_ctrl;

pub use int_ctrl::*;
use x86_64::structures::tss::TaskStateSegment;

use crate::{clock::AtomicClock, scheduling::Scheduler};
use lib::registers::msr::{Generic, IA32_KERNEL_GS_BASE};
use spin::{Mutex, MutexGuard};

#[repr(usize)]
pub enum StackIndex {
    TSS0,
    DoubleFault,
    Syscall,
    ExtINT,
}

struct LocalState<'a> {
    self_ptr: *mut Self,
    id: u32,
    clock: AtomicClock,
    int_ctrl: InterruptController,
    scheduler: Mutex<Scheduler>,
}

pub fn init() {
    assert_eq!(
        IA32_KERNEL_GS_BASE::read(),
        0,
        "Local state register has already been configured."
    );

    let id = lib::cpu::get_id();

    trace!("Configuring local state: {}.", id);
    unsafe {
        let ptr = lib::memory::malloc::get()
            .alloc(
                core::mem::size_of::<LocalState>(),
                // Local state register must be page-aligned.
                core::num::NonZeroUsize::new(core::mem::align_of::<LocalState>()),
            )
            .unwrap()
            .cast::<LocalState>()
            .unwrap()
            .into_parts()
            .0;

        trace!("Local state pointer: {:?}", ptr);

        {
            let clock = AtomicClock::new();
            let int_ctrl = InterruptController::create();
            let scheduler = Mutex::new(Scheduler::new());

            let tss = TaskStateSegment::new();

            assert_eq!(
                id,
                int_ctrl.apic_id() as u32,
                "CPUID processor ID does not match APIC ID."
            );

            // Write the LPU structure into memory.
            ptr.write(LocalState {
                self_ptr: ptr,
                id,
                clock,
                int_ctrl,
                scheduler,
            });
        }

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        IA32_KERNEL_GS_BASE::write(ptr as u64);
    }
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

pub unsafe fn get() -> &'static LocalState<'static> {
    let self_ptr: *const LocalState;

    core::arch::asm!(
        "mov {}, gs:0x0",
        out(reg) self_ptr,
        options(pure, nomem)
    );

    self_ptr.as_ref().expect("Local state not initialized")
}

pub unsafe fn id() -> u32 {
    get().id
}

pub unsafe fn clock() -> &'static AtomicClock {
    &get().clock
}

pub unsafe fn int_ctrl() -> &'static InterruptController {
    &get().int_ctrl
}

pub unsafe fn lock_scheduler() -> MutexGuard<'static, Scheduler> {
    get().scheduler.lock()
}

pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get().scheduler.try_lock()
}
