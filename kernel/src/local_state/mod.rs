mod int_ctrl;

pub use int_ctrl::*;

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::num::NonZeroUsize;
use lib::registers::msr::{Generic, IA32_KERNEL_GS_BASE};
use spin::{Mutex, MutexGuard};
use x86_64::structures::tss::TaskStateSegment;

pub const LOCAL_STATE_STACKS_OFFSET: usize = memoffset::offset_of!(LocalState, stacks);
pub const STACK_SIZE: usize = 0x2000;

#[repr(usize)]
pub enum StackIndex {
    Syscall,
}

#[repr(C, align(16))]
struct LocalState<'a> {
    self_ptr: *mut Self,
    id: u32,
    clock: AtomicClock,
    int_ctrl: InterruptController,
    scheduler: Mutex<Scheduler>,
    stacks: [*const u8; 1],
}

pub fn init() {
    assert_eq!(
        IA32_KERNEL_GS_BASE::read(),
        0,
        "Local state register has already been configured."
    );

    unsafe {
        let ptr = lib::memory::malloc::get()
            .alloc(
                core::mem::size_of::<LocalState>(),
                // Local state register must be page-aligned.
                NonZeroUsize::new(core::mem::align_of::<LocalState>()),
            )
            .unwrap()
            .cast::<LocalState>()
            .unwrap()
            .into_parts()
            .0;

        {
            let id = lib::cpu::get_id();
            let clock = AtomicClock::new();
            let int_ctrl = InterruptController::create();
            let scheduler = Mutex::new(Scheduler::new());
            let tss = TaskStateSegment::new();
            let syscall_stack = {
                let (ptr, len) = lib::memory::malloc::get()
                    .alloc(STACK_SIZE, NonZeroUsize::new(16))
                    .unwrap()
                    .into_parts();

                ptr.add(len)
            };

            // Write the LPU structure into memory.
            ptr.write(LocalState {
                self_ptr: ptr,
                id,
                clock,
                int_ctrl,
                scheduler,
                stacks: [syscall_stack],
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

unsafe fn get() -> &'static LocalState<'static> {
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
