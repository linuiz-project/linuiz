use crate::{
    exceptions::Exception,
    memory::{address_space::AddressSpace, ExactStack, PhysicalAllocator, KMALLOC},
    proc::{task::Task, Scheduler},
};
use core::{
    alloc::Allocator,
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};
use try_alloc::boxed::TryBox;

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub const SYSCALL_STACK_SIZE: usize = 0x4000;

pub enum ExceptionCatcher {
    Caught(Exception),
    Await,
    Idle,
}

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    syscall_stack_ptr: *const (),
    syscall_stack: ExactStack,

    magic: u64,
    core_id: u32,

    catching: AtomicBool,
    exception: UnsafeCell<Option<Exception>>,
    scheduler: Scheduler,

    #[cfg(target_arch = "x86_64")]
    idt: Option<TryBox<crate::arch::x64::structures::idt::InterruptDescriptorTable>>,
    #[cfg(target_arch = "x86_64")]
    tss: TryBox<crate::arch::x64::structures::tss::TaskStateSegment>,

    // TODO abstract this into some interrupt controller-esque structure
    #[cfg(target_arch = "x86_64")]
    apic: (apic::Apic, u64),
}

impl LocalState {
    const MAGIC: u64 = 0x1234_B33F_D3AD_C0DE;

    const fn is_valid_magic(&self) -> bool {
        self.magic == LocalState::MAGIC
    }
}

/// Returns the pointer to the local state structure.
#[inline]
fn get() -> &'static mut LocalState {
    #[cfg(target_arch = "x86_64")]
    {
        // Safety: If MSR is not null, then the `LocalState` has been initialized.
        match unsafe { (crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::read() as *mut LocalState).as_mut() } {
            Some(local_state) => local_state,
            None => panic!("kernel thread {} has not initialized local state", crate::cpu::read_id()),
        }
    }
}

/// Initializes the core-local state structure.
///
/// ### Safety
///
/// This function invariantly assumes it will only be called once.
#[allow(clippy::too_many_lines)]
pub unsafe fn init(core_id: u32, timer_frequency: u16) {
    let Ok(syscall_stack) = crate::memory::allocate_kernel_stack::<SYSCALL_STACK_SIZE>() else { crate::memory::out_of_memory() };
    let Ok(idle_task_stack) = crate::memory::allocate_kernel_stack::<0x10>() else { crate::memory::out_of_memory() };

    let local_state = LocalState {
        syscall_stack_ptr: syscall_stack.as_ptr().add(syscall_stack.len() & !0xF).cast(),
        syscall_stack,

        magic: LocalState::MAGIC,
        core_id,

        catching: AtomicBool::new(false),
        exception: UnsafeCell::new(None),
        scheduler: Scheduler::new(
            false,
            Task::new(0, || crate::interrupts::wait_loop(), idle_task_stack, crate::cpu::default_arch_context()),
        ),

        #[cfg(target_arch = "x86_64")]
        idt: {
            use crate::arch::x64::structures::idt;

            crate::init::get_parameters().low_memory.then(|| {
                let mut idt = TryBox::new(idt::InterruptDescriptorTable::new()).unwrap();

                idt::set_exception_handlers(&mut idt);
                idt::set_stub_handlers(&mut idt);
                idt.load_unsafe();

                idt
            })
        },
        #[cfg(target_arch = "x86_64")]
        tss: {
            use crate::arch::{
                reexport::x86_64::VirtAddr,
                x64::structures::{idt::StackTableIndex, tss},
            };
            use core::num::NonZeroUsize;

            let Ok(mut tss) = TryBox::new(tss::TaskStateSegment::new()) else { crate::memory::out_of_memory() };

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

            // TODO guard pages for these stacks ?
            tss.privilege_stack_table[0] = allocate_tss_stack(NonZeroUsize::new_unchecked(5));
            tss.interrupt_stack_table[StackTableIndex::Debug as usize] =
                allocate_tss_stack(NonZeroUsize::new_unchecked(2));
            tss.interrupt_stack_table[StackTableIndex::NonMaskable as usize] =
                allocate_tss_stack(NonZeroUsize::new_unchecked(2));
            tss.interrupt_stack_table[StackTableIndex::DoubleFault as usize] =
                allocate_tss_stack(NonZeroUsize::new_unchecked(2));
            tss.interrupt_stack_table[StackTableIndex::MachineCheck as usize] =
                allocate_tss_stack(NonZeroUsize::new_unchecked(2));

            tss::load_local(tss::ptr_as_descriptor(TryBox::as_nonnull_ptr(&tss)));

            tss
        },
        #[cfg(target_arch = "x86_64")]
        apic: {
            use crate::{arch::x64, interrupts::Vector};

            let apic =
                apic::Apic::new(Some(|address: usize| crate::memory::hhdm_address().as_ptr().add(address))).unwrap();

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

            (apic, timer_interval)
        },
    };

    let local_state_box =
        TryBox::new_in(local_state, &*crate::memory::PMM).expect("failed to allocate space for local state");
    let local_state_ptr = TryBox::leak(local_state_box) as *mut LocalState;
    trace!("Thread {} local state allocation: {:#X?}", core_id, local_state_ptr);

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(local_state_ptr.addr() as u64);
}

/// Safety
///
/// Caller must ensure control flow is prepared to begin scheduling tasks on the current core.
pub unsafe fn begin_scheduling() {
    let local_state = get();

    assert!(!local_state.scheduler.is_enabled());
    local_state.scheduler.enable();

    #[cfg(target_arch = "x86_64")]
    {
        assert!(local_state.apic.0.get_timer().get_masked());

        // Safety: Calling `begin_scheduling` implies this state change is expected.
        unsafe {
            local_state.apic.0.get_timer().set_masked(false);
        }
    }

    trace!("Core #{} scheduled.", local_state.core_id);

    // Safety: Calling `begin_scheduling` implies this function is expected to be called.
    unsafe { set_preemption_wait(core::num::NonZeroU16::MIN) };
}

/// Safety
///
/// Caller must ensure that context switching to a new task will not cause undefined behaviour.
pub unsafe fn next_task(ctrl_flow_context: &mut crate::cpu::Control, arch_context: &mut crate::cpu::ArchContext) {
    let local_state = get();
    local_state.scheduler.next_task(ctrl_flow_context, arch_context);
}

#[inline]
pub unsafe fn end_of_interrupt() {
    #[cfg(target_arch = "x86_64")]
    get().apic.0.end_of_interrupt();
}

/// Safety
///
/// Caller must ensure that setting a new preemption wait will not cause undefined behaviour.
pub unsafe fn set_preemption_wait(interval_wait: core::num::NonZeroU16) {
    #[cfg(target_arch = "x86_64")]
    {
        let (apic, timer_interval) = &get().apic;
        match apic.get_timer().get_mode() {
            // Safety: Control flow expects timer initial count to be set.
            apic::TimerMode::OneShot => unsafe {
                let final_count = timer_interval * u64::from(interval_wait.get());
                apic.set_timer_initial_count(final_count.try_into().unwrap_or(u32::MAX));
            },

            // Safety: Control flow expects the TSC deadline to be set.
            apic::TimerMode::TscDeadline => unsafe {
                crate::arch::x64::registers::msr::IA32_TSC_DEADLINE::set(
                    core::arch::x86_64::_rdtsc() + (timer_interval * (interval_wait.get() as u64)),
                );
            },

            apic::TimerMode::Periodic => unimplemented!(),
        }
    }
}

/// Allows safely running a function that manipulates the current task's address space, or returns `None` if there's no current task.
pub fn with_current_address_space<T>(with_fn: impl FnOnce(&mut AddressSpace<PhysicalAllocator>) -> T) -> Option<T> {
    get().scheduler.current_task().map(|task| crate::interrupts::without(|| task.with_address_space(with_fn)))
}

pub fn provide_exception<T: Into<Exception>>(exception: T) -> Result<(), T> {
    let local_state = get();

    if local_state.catching.load(Ordering::Relaxed) {
        debug_assert!(local_state.exception.get_mut().is_none());

        *local_state.exception.get_mut() = Some(exception.into());

        Ok(())
    } else {
        Err(exception)
    }
}

/// Safety
///
/// Caller must ensure `do_func` is effectively stackless, since no stack cleanup will occur on an exception.
pub unsafe fn do_catch<T>(do_func: impl FnOnce() -> T) -> Result<T, Exception> {
    let local_state = get();

    debug_assert!(local_state.exception.get_mut().is_none());

    local_state
        .catching
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .expect("nested exception catching is not supported");

    let do_func_result = do_func();
    let result = local_state.exception.get_mut().take().map_or(Ok(do_func_result), Err);

    local_state
        .catching
        .compare_exchange(true, false, Ordering::Relaxed, Ordering::Relaxed)
        .expect("inconsistent local catch state");

    result
}
