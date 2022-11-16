mod scheduler;

use core::alloc::Allocator;

use crate::memory::{Stack, VmemRegister, KMALLOC};
pub use scheduler::*;
use try_alloc::boxed::TryBox;

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub const SYSCALL_STACK_SIZE: usize = 0x4000;

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    syscall_stack_ptr: *const (),
    syscall_stack: Stack,
    magic: u64,
    core_id: u32,
    scheduler: Scheduler,

    #[cfg(target_arch = "x86_64")]
    idt: Option<&'static mut crate::arch::x64::structures::idt::InterruptDescriptorTable>,
    #[cfg(target_arch = "x86_64")]
    tss: TryBox<crate::arch::x64::structures::tss::TaskStateSegment>,
    #[cfg(target_arch = "x86_64")]
    apic: (apic::Apic, u64),
}

impl LocalState {
    const MAGIC: u64 = 0x1234_B33F_D3AD_C0DE;

    fn is_valid_magic(&self) -> bool {
        self.magic == LocalState::MAGIC
    }
}

/// Returns the pointer to the local state structure.
#[inline]
fn get() -> &'static mut LocalState {
    #[cfg(target_arch = "x86_64")]
    {
        // ### Safety: If MSR is not null, then the `LocalState` has been initialized.
        unsafe { ((crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::read()) as *mut LocalState).as_mut() }
            .unwrap()
    }
}

/// Initializes the core-local state structure.
///
/// ### Safety
///
/// This function invariantly assumes it will only be called once.
pub unsafe fn init(core_id: u32, timer_frequency: u16) {
    let Ok(syscall_stack) = crate::memory::allocate_kernel_stack::<SYSCALL_STACK_SIZE>() else { crate::memory::out_of_memory() };
    let Ok(idle_task_stack) = crate::memory::allocate_kernel_stack::<0x10>() else { crate::memory::out_of_memory() };

    let Ok(local_state_ptr) = lzalloc::allocate_with(|| {
        LocalState {
            syscall_stack_ptr: syscall_stack.as_ptr().add(syscall_stack.len() & !0xF).cast(),
            syscall_stack,
            magic: LocalState::MAGIC,
            core_id,
            scheduler: Scheduler::new(
                false,
                Task::new(
                    0,
                    EntryPoint::Function(crate::interrupts::wait_loop),
                            idle_task_stack,
                    {
                        #[cfg(target_arch = "x86_64")]
                        {
                            use crate::arch::x64;

                            (
                                x64::registers::GeneralRegisters::empty(),
                                x64::registers::SpecialRegisters::with_kernel_segments(
                                    x64::registers::RFlags::INTERRUPT_FLAG,
                                ),
                            )
                        }
                    },
                    VmemRegister::read(),
                ),
            ),

            #[cfg(target_arch = "x86_64")]
            idt: {
                use crate::arch::x64::structures::idt;

                // TODO use fallible allocations for this
                if !crate::PARAMETERS.low_memory {
                    Some({
                        let idt = lzalloc::allocate_with(|| idt::InterruptDescriptorTable::new()).unwrap().as_mut();

                        idt::set_exception_handlers(idt);
                        idt::set_stub_handlers(idt);
                        idt.load_unsafe();

                        idt
                    })
                } else {
                    None
                }
            },
            #[cfg(target_arch = "x86_64")]
            tss: {
                use crate::arch::{reexport::x86_64::VirtAddr, x64::structures::{tss, idt::StackTableIndex}};
                use core::num::NonZeroUsize;

                let Ok(tss) = TryBox::new(tss::TaskStateSegment::new()) else { crate::memory::out_of_memory() };

                fn allocate_tss_stack(pages: NonZeroUsize) -> VirtAddr {
                    VirtAddr::from_ptr(
                crate::memory::KMALLOC.allocate(
                    // ### Safety: Values provided are known-valid.
                            unsafe {
                                core::alloc::Layout::from_size_align_unchecked(pages.get() * 0x1000, 0x10)
                            },
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

                tss_ptr.as_mut()
            },
            #[cfg(target_arch = "x86_64")]
            apic: {
                use crate::{arch::x64, interrupts::Vector};

                let apic =
                    apic::Apic::new(Some(|address: usize| {

                        libcommon::Address::<libcommon::Virtual>::new_truncate(crate::memory::get_hhdm_address().as_u64() + (address as u64)).as_mut_ptr()
                    }))
                    .unwrap();

                // Bring APIC to known state.
                apic.software_reset(255, 254, 253);
                apic.get_timer().set_vector(Vector::Timer as u8);
                apic.get_error().set_vector(Vector::Error as u8).set_masked(false);
                apic.get_performance().set_vector(Vector::Performance as u8).set_masked(true);
                apic.get_thermal_sensor().set_vector(Vector::Thermal as u8).set_masked(true);

                // Configure APIC timer in most advanced mode.
                let timer_interval = if x64::cpuid::FEATURE_INFO.has_tsc()
                    && x64::cpuid::FEATURE_INFO.has_tsc_deadline()
                {
                    apic.get_timer().set_mode(apic::TimerMode::TscDeadline);

                    let frequency = x64::cpuid::CPUID.get_processor_frequency_info().map_or_else(
                        || {
                            libcommon::do_once!({
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
        }
    }) else {
        panic!("Failed to allocate space for local state.")
    };

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(local_state_ptr.addr().get() as u64);
}

/// ### Safety
///
/// Caller must ensure control flow is prepared to begin scheduling tasks on the current core.
pub unsafe fn begin_scheduling() {
    let local_state = get();

    assert!(!local_state.scheduler.is_enabled());
    local_state.scheduler.enable();

    // ### Safety: Value provided is non-zero, and enable/reload is expected / appropriate.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        local_state.apic.0.get_timer().set_masked(false);
    }

    trace!("Core #{} scheduled.", local_state.core_id);

    // ### Safety: Value provided is non-zero.
    preemption_wait(core::num::NonZeroU16::new_unchecked(1));
}

/// ### Safety
///
/// Caller must ensure that context switching to a new task will not cause undefined behaviour.
pub unsafe fn next_task(
    ctrl_flow_context: &mut crate::cpu::ControlContext,
    arch_context: &mut crate::cpu::ArchContext,
) {
    let local_state = get();
    local_state.scheduler.next_task(ctrl_flow_context, arch_context);
}

#[inline]
pub unsafe fn end_of_interrupt() {
    #[cfg(target_arch = "x86_64")]
    get().apic.0.end_of_interrupt()
}

/// ### Safety
///
/// Caller must ensure that setting a new preemption wait will not cause undefined behaviour.
pub unsafe fn preemption_wait(interval_wait: core::num::NonZeroU16) {
    #[cfg(target_arch = "x86_64")]
    {
        let (apic, timer_interval) = &get().apic;
        match apic.get_timer().get_mode() {
            // ### Safety: Control flow expects timer initial count to be changed.
            apic::TimerMode::OneShot => unsafe {
                apic.set_timer_initial_count((timer_interval * (interval_wait.get() as u64)) as u32)
            },
            apic::TimerMode::TscDeadline => unsafe {
                crate::arch::x64::registers::msr::IA32_TSC_DEADLINE::set(
                    core::arch::x86_64::_rdtsc() + (timer_interval * (interval_wait.get() as u64)),
                )
            },
            apic::TimerMode::Periodic => unimplemented!(),
        }
    }
}
