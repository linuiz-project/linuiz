use crate::{
    interrupts::{InterruptCell, exceptions::Exception},
    mem::alloc::KERNEL_ALLOCATOR,
    task::Scheduler,
    time::LocalTimer,
};
use alloc::alloc::Allocator;
use core::{cell::UnsafeCell, ptr::NonNull, sync::atomic::AtomicBool, time::Duration};
use spin::Mutex;

pub const STACK_SIZE: usize = 0x10000;
pub const SYSCALL_STACK_SIZE: usize = 0x40000;

pub enum ExceptionCatcher {
    Caught(Exception),
    Await,
    Idle,
}

fn try_get_local_static_ptr() -> Option<NonNull<LocalState>> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::model_specific::IA32_KERNEL_GS_BASE::get_local_state_ptr()
    }
}

/// Local (to the current processor) state structure.
#[repr(align(0x1000))]
pub struct LocalState {
    timer: LocalTimer,
    scheduler: InterruptCell<Mutex<Scheduler>>,
    catch_exception: AtomicBool,
    exception: UnsafeCell<Option<Exception>>,
}

impl LocalState {
    /// Initializes the local state structure.
    pub fn init() {
        assert!(
            try_get_local_static_ptr().is_none(),
            "local state has already been initialized"
        );

        trace!("Configuring local timer...");
        let timer = LocalTimer::configure();

        trace!("Configuring local scheduler...");
        let scheduler = Scheduler::new();

        let local_state_ptr = KERNEL_ALLOCATOR
            .allocate(core::alloc::Layout::new::<Self>())
            .expect("failed to allocate local state")
            .as_non_null_ptr()
            .cast::<Self>();

        let local_state = LocalState {
            timer,
            scheduler: InterruptCell::new(Mutex::new(scheduler)),
            catch_exception: AtomicBool::new(false),
            exception: UnsafeCell::new(None),
        };

        // Safety: Memory was allocated for the size and align of `Self`.
        unsafe {
            local_state_ptr.write(local_state);
        }

        // Set the local state pointer for this processor.
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::model_specific::IA32_KERNEL_GS_BASE;

                assert!(IA32_KERNEL_GS_BASE::get_local_state_ptr().is_none());

                unsafe {
                    IA32_KERNEL_GS_BASE::set_local_state_ptr(local_state_ptr);
                }
            }

            _ => { todo!() }
        }

        debug!("Local state has been initialized.");
    }

    /// Gets the local processor state structure.
    fn get_local_static() -> &'static Self {
        try_get_local_static_ptr()
            .map(|local_state_ptr| {
                // Safety: If the state pointer is non-null, the kernel guarantees it will be
                // valid for reading as `LocalState`.
                unsafe { local_state_ptr.as_ref() }
            })
            .expect("local state has not been initialized")
    }

    pub fn with_scheduler<T>(func: impl FnOnce(&mut Scheduler) -> T) -> T {
        Self::get_local_static().scheduler.with(|scheduler| {
            let mut scheduler = scheduler.lock();

            func(&mut scheduler)
        })
    }

    /// ## Safety
    ///
    /// - Function should only be called once the last preemption wait has
    ///   resolved.
    pub unsafe fn set_preemption_wait(duration: Duration) {
        LocalState::get_local_static()
            .timer
            .set_wait(duration)
            .expect("preemption wait duration was too long");
    }
}

// /// TODO inline this function
// pub unsafe fn begin_scheduling() {
//     // Enable scheduler ...
//     with_scheduler(|scheduler| {
//         assert!(!scheduler.is_enabled());
//         scheduler.enable();
//     });

//     // Enable APIC timer ...
//     // TODO APIC
//     // let apic = &mut get_mut().apic;
//     // assert!(apic.get_timer().get_masked());
//     // // Safety: Calling `begin_scheduling` implies this state change is
// expected.     // unsafe {
//     //     apic.get_timer().set_masked(false);
//     // }

//     // Safety: Calling `begin_scheduling` implies this function is expected
// to be called.     unsafe {
//         set_preemption_wait(core::num::NonZeroU16::MIN);
//     }
// }

// pub fn provide_exception<T: Into<Exception>>(exception: T) ->
// core::result::Result<(), T> {     let state = get_state_mut();
//     if state.catch_exception.load(Ordering::Relaxed) {
//         let exception_cell = state.exception.get_mut();

//         debug_assert!(exception_cell.is_none());
//         *exception_cell = Some(exception.into());
//         Ok(())
//     } else {
//         Err(exception)
//     }
// }

// /// ## Safety
// ///
// /// Caller must ensure `do_func` is effectively stackless, since no stack
// cleanup will occur on an exception. pub unsafe fn do_catch<T>(do_func: impl
// FnOnce() -> T) -> core::result::Result<T, Exception> {     let state =
// get_state_mut();

//     debug_assert!(state.exception.get_mut().is_none());

//     state
//         .catch_exception
//         .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
//         .expect("nested exception catching is not supported");

//     let do_func_result = do_func();
//     let result = state.exception.get_mut().take().map_or(Ok(do_func_result),
// Err);

//     state
//         .catch_exception
//         .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
//         .expect("inconsistent local catch state");

//     result
// }
