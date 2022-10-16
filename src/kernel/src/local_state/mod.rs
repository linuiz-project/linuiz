mod scheduler;

use crate::memory::VmemRegister;
pub use scheduler::*;

pub(self) const US_PER_SEC: u32 = 1000000;
pub(self) const US_WAIT: u32 = 10000;
pub(self) const US_FREQ_FACTOR: u32 = US_PER_SEC / US_WAIT;

pub const SYSCALL_STACK_SIZE: usize = 0x4000;

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    syscall_stack_ptr: *const (),
    padding: [u8; 0x8],
    syscall_stack: [u8; SYSCALL_STACK_SIZE],
    magic: u64,
    core_id: u32,
    scheduler: Scheduler,

    #[cfg(target_arch = "x86_64")]
    idt: Option<&'static mut crate::arch::x64::structures::idt::InterruptDescriptorTable>,
    #[cfg(target_arch = "x86_64")]
    tss: &'static mut crate::arch::x64::structures::tss::TaskStateSegment,
    #[cfg(target_arch = "x86_64")]
    apic: (crate::arch::x64::structures::apic::Apic, u64),
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
        // SAFETY: If MSR is not null, then the `LocalState` has been initialized.
        unsafe { ((crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::read()) as *mut LocalState).as_mut() }
            .unwrap()
    }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init(core_id: u32, timer_frequency: u16) {
    let local_state_ptr = alloc::alloc::alloc(core::alloc::Layout::from_size_align_unchecked(
        core::mem::size_of::<LocalState>(),
        core::mem::align_of::<LocalState>(),
    ))
    .cast::<LocalState>();
    assert!(!local_state_ptr.is_null());

    local_state_ptr.write(LocalState {
        syscall_stack_ptr: core::ptr::null(),
        padding: [0u8; 0x8],
        syscall_stack: [0u8; SYSCALL_STACK_SIZE],
        magic: LocalState::MAGIC,
        core_id,
        scheduler: Scheduler::new(
            false,
            Task::new(
                TaskPriority::new(1).unwrap(),
                TaskStart::Function(crate::interrupts::wait_loop),
                TaskStack::At(libcommon::Address::<libcommon::Virtual>::from_ptr({
                    alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align_unchecked(0x10, 0x10))
                })),
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

            if !crate::PARAMETERS.low_memory
                && let Ok(idt_ptr) = libcommon::memory::allocate_static_zeroed::<idt::InterruptDescriptorTable>() {
                Some({
                    let idt = &mut *idt_ptr;

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
            use crate::arch::x64::structures::{gdt, tss};

            let tss_ptr = {
                use crate::arch::{reexport::x86_64::VirtAddr, x64::structures::idt::StackTableIndex};
                use core::num::NonZeroUsize;
                use libcommon::memory::allocate_static_zeroed;

                let ptr = allocate_static_zeroed::<tss::TaskStateSegment>().unwrap();

                fn allocate_tss_stack(pages: NonZeroUsize) -> VirtAddr {
                    VirtAddr::from_ptr({
                        // SAFETY: Values provided are known-valid.
                        let layout =
                            unsafe { core::alloc::Layout::from_size_align_unchecked(pages.get() * 0x1000, 0x10) };
                        // SAFETY: Layout provided has a known-non-zero size.
                        unsafe { alloc::alloc::alloc(layout) }
                    })
                }

                let tss = &mut *ptr;
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

                ptr
            };

            let tss_descriptor = {
                use bit_field::BitField;

                let tss_ptr_u64 = tss_ptr as usize as u64;

                let mut low = gdt::DescriptorFlags::PRESENT.bits();
                // base
                low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
                low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
                // limit (the `-1` is needed since the bound is inclusive, not exclusive)
                low.set_bits(0..16, (core::mem::size_of::<tss::TaskStateSegment>() - 1) as u64);
                // type (0b1001 = available 64-bit tss)
                low.set_bits(40..44, 0b1001);

                // high 32 bits of base
                let mut high = 0;
                high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

                gdt::Descriptor::SystemSegment(low, high)
            };

            // Store current GDT pointer to restore later.
            let cur_gdt = gdt::sgdt();
            // Create temporary kernel GDT to avoid a GPF on switching to it.
            let mut temp_gdt = gdt::GlobalDescriptorTable::new();
            temp_gdt.add_entry(gdt::Descriptor::kernel_code_segment());
            temp_gdt.add_entry(gdt::Descriptor::kernel_data_segment());
            let tss_selector = temp_gdt.add_entry(tss_descriptor);

            // Load temp GDT ...
            temp_gdt.load_unsafe();
            // ... load TSS from temporary GDT ...
            tss::load_tss(tss_selector);
            // ... and restore cached GDT.
            gdt::lgdt(&cur_gdt);

            &mut *tss_ptr
        },
        #[cfg(target_arch = "x86_64")]
        apic: {
            use crate::{arch::x64, interrupts::Vector};

            let apic = x64::structures::apic::Apic::new(Some(|address: libcommon::Address<libcommon::Physical>| {
                use libcommon::{Address, Page, Virtual};

                let page_address = Address::<Page>::new(
                    Address::<Virtual>::new(crate::memory::get_hhdm_address().as_u64() + address.as_u64()).unwrap(),
                    Some(libcommon::PageAlign::Align4KiB),
                )
                .unwrap();

                crate::memory::get_kernel_mapper()
                    .map_if_not_mapped(
                        page_address,
                        Some((address.frame(), false)),
                        crate::memory::PageAttributes::MMIO,
                    )
                    .unwrap();

                page_address.address()
            }))
            .unwrap();

            // Bring APIC to known state.
            apic.software_reset(255, 254, 253);
            apic.get_timer().set_vector(Vector::Timer as u8);
            apic.get_error().set_vector(Vector::Error as u8).set_masked(false);
            apic.get_performance().set_vector(Vector::Performance as u8).set_masked(true);
            apic.get_thermal_sensor().set_vector(Vector::Thermal as u8).set_masked(true);

            // Configure APIC timer in most advanced mode.
            let timer_interval = if x64::cpuid::FEATURE_INFO.has_tsc() && x64::cpuid::FEATURE_INFO.has_tsc_deadline() {
                apic.get_timer().set_mode(x64::structures::apic::TimerMode::TscDeadline);

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
                apic.set_timer_divisor(x64::structures::apic::TimerDivisor::Div1);
                apic.get_timer().set_masked(true).set_mode(x64::structures::apic::TimerMode::OneShot);

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
    });
    // Write out correct syscall stack pointer.
    local_state_ptr.cast::<*const ()>().write(local_state_ptr.cast::<u8>().add(8 + SYSCALL_STACK_SIZE).cast());

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(local_state_ptr as usize as u64);
}

pub fn with_scheduler<T>(func: impl FnOnce(&mut Scheduler) -> T) -> T {
    crate::interrupts::without(|| func(&mut get().scheduler))
}

// TODO remove this, scheduling always enabled when local state init is done
// SAFETY: Caller must ensure control flow is prepared to begin scheduling tasks on the current core.
pub unsafe fn begin_scheduling() {
    let local_state = get();

    assert!(!local_state.scheduler.is_enabled());
    local_state.scheduler.enable();

    // SAFETY: Value provided is non-zero, and enable/reload is expected / appropriate.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        local_state.apic.0.get_timer().set_masked(false);
    }

    // SAFETY: Value provided is non-zero.
    preemption_wait(unsafe { core::num::NonZeroU16::new_unchecked(1) });
}

pub fn next_task(ctrl_flow_context: &mut crate::cpu::ControlContext, arch_context: &mut crate::cpu::ArchContext) {
    get().scheduler.next_task(ctrl_flow_context, arch_context);
}

#[inline]
pub fn end_of_interrupt() {
    #[cfg(target_arch = "x86_64")]
    get().apic.0.end_of_interrupt()
}

pub fn preemption_wait(interval_wait: core::num::NonZeroU16) {
    #[cfg(target_arch = "x86_64")]
    {
        use crate::arch::x64::structures::apic;

        let (apic, timer_interval) = &get().apic;
        match apic.get_timer().get_mode() {
            // SAFETY: Control flow expects timer initial count to be changed.
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
