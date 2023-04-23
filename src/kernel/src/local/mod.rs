use crate::{exceptions::Exception, memory::alloc::KMALLOC, proc::Scheduler};
use alloc::boxed::Box;
use core::{
    alloc::Allocator,
    cell::UnsafeCell,
    mem::MaybeUninit,
    num::NonZeroU64,
    ptr::NonNull,
    sync::atomic::{AtomicBool, Ordering},
};

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub const STACK_SIZE: usize = 0x10000;

#[repr(C)]
struct State {
    syscall_stack_ptr: NonNull<MaybeUninit<u8>>,
    syscall_stack: Box<[MaybeUninit<u8>]>,

    core_id: u32,
    scheduler: Scheduler,

    #[cfg(target_arch = "x86_64")]
    idt: Box<crate::arch::x64::structures::idt::InterruptDescriptorTable>,
    #[cfg(target_arch = "x86_64")]
    tss: Box<crate::arch::x64::structures::tss::TaskStateSegment>,

    #[cfg(target_arch = "x86_64")]
    apic: apic::Apic,

    timer_interval: Option<NonZeroU64>,

    catch_exception: AtomicBool,
    exception: UnsafeCell<Option<Exception>>,
}

pub const SYSCALL_STACK_SIZE: usize = 0x40000;

pub enum ExceptionCatcher {
    Caught(Exception),
    Await,
    Idle,
}

// TODO
//
//

/// Initializes the core-local state structure.
///
/// ### Safety
///
/// This function invariantly assumes it will only be called once.
#[allow(clippy::too_many_lines)]
pub unsafe fn init(timer_frequency: u16) {
    let syscall_stack = Box::new_zeroed_slice(STACK_SIZE);

    #[cfg(target_arch = "x86_64")]
    let idt = {
        use crate::arch::x64::structures::idt;

        let mut idt = Box::new(idt::InterruptDescriptorTable::new());

        idt::set_exception_handlers(&mut idt);
        idt::set_stub_handlers(&mut idt);
        idt.load_unsafe();

        idt
    };

    #[cfg(target_arch = "x86_64")]
    let tss = {
        use crate::arch::{
            reexport::x86_64::VirtAddr,
            x64::structures::{idt::StackTableIndex, tss},
        };
        use core::num::NonZeroUsize;

        fn allocate_tss_stack(pages: NonZeroUsize) -> VirtAddr {
            VirtAddr::from_ptr(
                KMALLOC
                    .allocate(
                        // Safety: Values provided are known-valid.
                        unsafe { core::alloc::Layout::from_size_align_unchecked(pages.get() * 0x1000, 0x10) },
                    )
                    .unwrap()
                    .as_non_null_ptr()
                    .as_ptr(),
            )
        }

        let mut tss = Box::new(tss::TaskStateSegment::new());

        // TODO guard pages for these stacks ?
        tss.privilege_stack_table[0] = allocate_tss_stack(NonZeroUsize::new_unchecked(8));
        tss.interrupt_stack_table[StackTableIndex::Debug as usize] = allocate_tss_stack(NonZeroUsize::new_unchecked(2));
        tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] =
            allocate_tss_stack(NonZeroUsize::new_unchecked(2));
        tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] =
            allocate_tss_stack(NonZeroUsize::new_unchecked(2));
        tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] =
            allocate_tss_stack(NonZeroUsize::new_unchecked(2));

        tss::load_local(tss::ptr_as_descriptor(NonNull::new(&raw mut *tss).unwrap()));

        tss
    };

    let mut state = Box::new(State {
        syscall_stack_ptr: NonNull::new(syscall_stack.as_ptr_range().end.cast_mut()).unwrap(),
        syscall_stack,

        core_id: crate::cpu::read_id(),
        scheduler: Scheduler::new(false),

        #[cfg(target_arch = "x86_64")]
        idt,
        #[cfg(target_arch = "x86_64")]
        tss,

        #[cfg(target_arch = "x86_64")]
        apic: apic::Apic::new(Some(|address: usize| crate::memory::Hhdm::ptr().add(address))).unwrap(),

        timer_interval: None,

        catch_exception: AtomicBool::new(false),
        exception: UnsafeCell::new(None),
    });

    /* init IDT */
    {}

    /* init TSS */

    /* init APIC */
    {
        use crate::{arch::x64, interrupts::Vector};

        let apic = &mut state.apic;

        // Bring APIC to known state.
        apic.software_reset(255, 254, 253);
        apic.get_timer().set_vector(Vector::Timer as u8);
        apic.get_error().set_vector(Vector::Error as u8).set_masked(false);
        apic.get_performance().set_vector(Vector::Performance as u8).set_masked(true);
        apic.get_thermal_sensor().set_vector(Vector::Thermal as u8).set_masked(true);

        // Configure APIC timer in most advanced mode.
        let timer_interval = if x64::cpuid::FEATURE_INFO.has_tsc() && x64::cpuid::FEATURE_INFO.has_tsc_deadline() {
            apic.get_timer().set_mode(apic::TimerMode::TscDeadline);

            let frequency = x64::cpuid::CPUID.get_processor_frequency_info().map_or_else(
                || {
                    libsys::do_once!({
                        trace!("Processors do not support TSC frequency reporting via CPUID.");
                    });

                    apic.sw_enable();
                    apic.get_timer().set_masked(true);

                    let start_tsc = core::arch::x86_64::_rdtsc();
                    crate::time::SYSTEM_CLOCK.spin_wait_us(US_WAIT);
                    let end_tsc = core::arch::x86_64::_rdtsc();

                    (end_tsc - start_tsc) * (US_FREQ_FACTOR as u64)
                },
                |info| {
                    (info.bus_frequency() as u64)
                        / ((info.processor_base_frequency() as u64) * (info.processor_max_frequency() as u64))
                },
            );

            frequency / (timer_frequency as u64)
        } else {
            apic.sw_enable();
            apic.set_timer_divisor(apic::TimerDivisor::Div1);
            apic.get_timer().set_masked(true).set_mode(apic::TimerMode::OneShot);

            let frequency = {
                apic.set_timer_initial_count(u32::MAX);
                crate::time::SYSTEM_CLOCK.spin_wait_us(US_WAIT);
                let timer_count = apic.get_timer_current_count();

                (u32::MAX - timer_count) * US_FREQ_FACTOR
            };

            // Ensure we reset the APIC timer to avoid any errant interrupts.
            apic.set_timer_initial_count(0);

            (frequency / (timer_frequency as u32)) as u64
        };

        state.timer_interval = NonZeroU64::new(timer_interval);
    }

    state.syscall_stack_ptr = NonNull::new(state.syscall_stack.as_mut_ptr().add(state.syscall_stack.len())).unwrap();
    let state_ptr = Box::leak(state) as *mut State;
    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(state_ptr.addr() as u64);
}

fn get_state_ptr() -> Option<NonNull<State>> {
    let kernel_gs_usize = usize::try_from(crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::read()).unwrap();
    NonNull::new(kernel_gs_usize as *mut State)
}

fn get_state() -> &'static State {
    unsafe { get_state_ptr().expect("core state uninitialized").as_ref() }
}

fn get_state_mut() -> &'static mut State {
    unsafe { get_state_ptr().expect("core state uninitialized").as_mut() }
}

/// Returns the generated ID for the local core.
pub fn get_core_id() -> u32 {
    get_state().core_id
}

pub unsafe fn begin_scheduling() {
    let state = get_state_mut();

    // Enable scheduler ...
    let scheduler = &mut state.scheduler;
    assert!(!scheduler.is_enabled());
    scheduler.enable();

    // Enable APIC timer ...
    let apic = &mut state.apic;
    assert!(apic.get_timer().get_masked());
    // Safety: Calling `begin_scheduling` implies this state change is expected.
    unsafe {
        apic.get_timer().set_masked(false);
    }

    // Safety: Calling `begin_scheduling` implies this function is expected to be called.
    unsafe {
        set_preemption_wait(core::num::NonZeroU16::MIN);
    }
}

pub fn with_scheduler<R>(func: impl FnOnce(&mut crate::proc::Scheduler) -> R) -> R {
    func(&mut get_state_mut().scheduler)
}

pub fn print_timer_interval(id: usize) {
    info!("TIMER INTERVAL {} {:?}", id, get_state().timer_interval);
}

pub unsafe fn end_of_interrupt() {
    #[cfg(target_arch = "x86_64")]
    get_state().apic.end_of_interrupt();
}

/// ### Safety
///
/// Caller must ensure that setting a new preemption wait will not cause undefined behaviour.
pub unsafe fn set_preemption_wait(interval_wait: core::num::NonZeroU16) {
    let state = get_state_mut();
    let timer_interval = state.timer_interval.unwrap();

    #[cfg(target_arch = "x86_64")]
    {
        let apic = &mut state.apic;

        match apic.get_timer().get_mode() {
            // Safety: Control flow expects timer initial count to be set.
            apic::TimerMode::OneShot => unsafe {
                info!("{} * {}", timer_interval.get(), interval_wait.get());
                let final_count = timer_interval.get() * u64::from(interval_wait.get());
                apic.set_timer_initial_count(final_count.try_into().unwrap_or(u32::MAX));
            },

            // Safety: Control flow expects the TSC deadline to be set.
            apic::TimerMode::TscDeadline => unsafe {
                crate::arch::x64::registers::msr::IA32_TSC_DEADLINE::set(
                    core::arch::x86_64::_rdtsc() + (timer_interval.get() * u64::from(interval_wait.get())),
                );
            },

            apic::TimerMode::Periodic => unimplemented!(),
        }
    }
}

pub fn provide_exception<T: Into<Exception>>(exception: T) -> Result<(), T> {
    let state = get_state_mut();
    if state.catch_exception.load(Ordering::Relaxed) {
        let exception_cell = state.exception.get_mut();

        debug_assert!(exception_cell.is_none());
        *exception_cell = Some(exception.into());
        Ok(())
    } else {
        Err(exception)
    }
}

/// ### Safety
///
/// Caller must ensure `do_func` is effectively stackless, since no stack cleanup will occur on an exception.
pub unsafe fn do_catch<T>(do_func: impl FnOnce() -> T) -> Result<T, Exception> {
    let state = get_state_mut();

    debug_assert!(state.exception.get_mut().is_none());

    state
        .catch_exception
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .expect("nested exception catching is not supported");

    let do_func_result = do_func();
    let result = state.exception.get_mut().take().map_or(Ok(do_func_result), Err);

    state
        .catch_exception
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .expect("inconsistent local catch state");

    result
}
