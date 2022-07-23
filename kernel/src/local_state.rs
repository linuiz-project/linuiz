use crate::{
    clock::AtomicClock,
    scheduling::{Task, TaskPriority},
};
use core::sync::atomic::{AtomicUsize, Ordering};
use liblz::{
    registers::{control::CR3, RFlags},
    Address, Virtual,
};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    GlobalTimer = 32,
    LocalTimer = 48,
    CMCI = 49,
    Performance = 50,
    ThermalSensor = 51,
    Error = 54,
    Storage = 55,
}

#[repr(align(0x1000))]
struct LocalState {
    magic: u64,
    clock: AtomicClock,
    default_task: Task,
    cur_task: Option<Task>,
    local_timer_per_ms: u32,
    syscall_stack: [u8; 0x4000],
    privilege_stack: [u8; 0x4000],
}

impl LocalState {
    const MAGIC: u64 = 0xFFFF_D3ADC0DE_FFFF;

    fn validate_init(&self) {
        debug_assert!(self.magic == LocalState::MAGIC);
    }
}

static LOCAL_STATE_PTRS_BASE: AtomicUsize = AtomicUsize::new(0);

/// Returns the pointer to the local state structure.
///
/// SAFETY: It is important that, prior to utilizing the value returned by
///         this function, it is ensured that the memory it refers to is
///         actually mapped via the virtual memory manager. No guarantees
///         are made as to whether this has been done or not.
#[inline(always)]
unsafe fn get_local_state_ptr() -> *mut LocalState {
    (LOCAL_STATE_PTRS_BASE.load(Ordering::Relaxed) as *mut LocalState)
        // TODO move to a core-local PML4 copy with an L4 local state mapping
        .add(liblz::structures::apic::get_id() as usize)
}

#[inline]
fn local_state() -> &'static mut LocalState {
    unsafe { get_local_state_ptr().as_mut().unwrap() }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn create() {
    LOCAL_STATE_PTRS_BASE
        .compare_exchange(
            0,
            // Cosntruct the local state pointer (with slide) via the `Address` struct, to
            // automatically sign extend.
            Address::<Virtual>::new(
                ((510 * liblz::memory::PML4_ENTRY_MEM_SIZE)
                    + (liblz::instructions::rdrand32().unwrap() as usize))
                    & !0xFFF,
            )
            .as_usize(),
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .ok();

    let local_state_ptr = get_local_state_ptr();

    {
        use liblz::memory::Page;

        // Map the pages this local state will utilize.
        let page_manager = liblz::memory::global_pmgr();
        let base_page = Page::from_ptr(local_state_ptr);
        let end_page = Page::from_ptr(local_state_ptr.add(liblz::align_up_div(
            core::mem::size_of::<LocalState>(),
            0x1000,
        )));
        (base_page..end_page)
            .for_each(|page| page_manager.auto_map(&page, liblz::memory::PageAttributes::DATA));
    }

    /* CONFIGURE APIC */
    use liblz::structures::apic;

    apic::software_reset();
    apic::set_timer_divisor(apic::TimerDivisor::Div1);
    apic::get_timer()
        .set_mode(apic::TimerMode::OneShot)
        .set_vector(InterruptVector::LocalTimer as u8);
    apic::get_error()
        .set_vector(InterruptVector::Error as u8)
        .set_masked(false);
    crate::tables::idt::set_handler_fn(InterruptVector::LocalTimer as u8, local_clock_tick);
    apic::get_performance().set_vector(InterruptVector::Performance as u8);
    apic::get_thermal_sensor().set_vector(InterruptVector::ThermalSensor as u8);
    // LINT0&1 should be configured by the APIC reset.

    // Ensure interrupts are completely enabled after APIC is reset.
    liblz::registers::msr::IA32_APIC_BASE::set_hw_enable(true);
    liblz::instructions::interrupts::enable();
    liblz::structures::apic::sw_enable();

    let per_ms =
        {
            if let Some(registers) = liblz::instructions::cpuid::exec(0x15, 0x0)
                .and_then(|result| if result.ebx() > 0 { Some(result) } else { None })
            // Attempt to calculate a concrete frequency via CPUID.
            {
                let per_ms = registers.ecx() * (registers.ebx() / registers.eax());
                trace!("CPU clock frequency reporting: {} Hz", per_ms);
                per_ms
            } else
            // Otherwise, determine frequency with external measurements.
            {
                trace!("CPU does not support clock frequency reporting via CPUID.");

                const MS_WINDOW: u32 = 10;
                // Wait on the global timer, to ensure we're starting the count
                // on the rising edge of each millisecond.
                crate::clock::global::busy_wait_msec(1);
                apic::set_timer_initial_count(u32::MAX);
                crate::clock::global::busy_wait_msec(MS_WINDOW as u64);

                let per_ms = (u32::MAX - apic::get_timer_current_count()) / MS_WINDOW;
                trace!(
                    "CPU clock frequency measurement: {} Hz",
                    per_ms * (1000 / MS_WINDOW)
                );
                per_ms
            }
        };

    local_state_ptr.write_volatile(LocalState {
        magic: LocalState::MAGIC,
        clock: AtomicClock::new(),
        default_task: Task::new(
            TaskPriority::new(1).unwrap(),
            liblz::instructions::hlt_indefinite,
            None,
            RFlags::INTERRUPT_FLAG,
            *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
            *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
            CR3::read(),
        ),
        cur_task: None,
        local_timer_per_ms: per_ms,
        syscall_stack: [0u8; 0x4000],
        privilege_stack: [0u8; 0x4000],
    });
    local_state().validate_init();
}

fn local_clock_tick(
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    cached_regs: *mut crate::scheduling::ThreadRegisters,
) {
    use crate::scheduling::SCHEDULER;

    const MIN_TIME_SLICE_MS: u32 = 1;
    const PRIO_TIME_SLICE_MS: u32 = 2;

    let local_state = local_state();
    local_state.clock.tick();

    if let Some(mut cur_task) = local_state.cur_task.take() {
        cur_task.rip = stack_frame.instruction_pointer.as_u64();
        cur_task.cs = stack_frame.code_segment as u16;
        cur_task.rsp = stack_frame.stack_pointer.as_u64();
        cur_task.ss = stack_frame.stack_segment as u16;
        cur_task.rfl = RFlags::from_bits_truncate(stack_frame.cpu_flags);
        cur_task.gprs = unsafe { cached_regs.read_volatile() };
        cur_task.cr3 = CR3::read();

        SCHEDULER.push_task(cur_task);
    }

    unsafe {
        if let Some(next_task) = crate::scheduling::SCHEDULER.pop_task() {
            // Modify task frame to restore rsp & rip.
            stack_frame
                .as_mut()
                .write(x86_64::structures::idt::InterruptStackFrameValue {
                    instruction_pointer: x86_64::VirtAddr::new_truncate(next_task.rip),
                    code_segment: next_task.cs as u64,
                    cpu_flags: next_task.rfl.bits(),
                    stack_pointer: x86_64::VirtAddr::new_truncate(next_task.rsp),
                    stack_segment: next_task.ss as u64,
                });

            // Restore task registers.
            cached_regs.write_volatile(next_task.gprs);

            // Set current page tables.
            CR3::write(next_task.cr3.0, next_task.cr3.1);

            let next_timer_ms = (next_task.prio().get() as u32) * PRIO_TIME_SLICE_MS;
            local_state.cur_task = Some(next_task);

            reload_timer(core::num::NonZeroU32::new(next_timer_ms).unwrap());
            liblz::structures::apic::end_of_interrupt();
        } else {
            let default_task = &local_state.default_task;

            stack_frame
                .as_mut()
                .write(x86_64::structures::idt::InterruptStackFrameValue {
                    instruction_pointer: x86_64::VirtAddr::new_truncate(default_task.rip),
                    code_segment: default_task.cs as u64,
                    cpu_flags: default_task.rfl.bits(),
                    stack_pointer: x86_64::VirtAddr::new_truncate(default_task.rsp),
                    stack_segment: default_task.ss as u64,
                });

            // Set current page tables.
            CR3::write(default_task.cr3.0, default_task.cr3.1);

            reload_timer(core::num::NonZeroU32::new(MIN_TIME_SLICE_MS).unwrap());
            liblz::structures::apic::end_of_interrupt();
        }
    }
}

#[inline]
pub fn clock() -> &'static AtomicClock {
    &local_state().clock
}

/// SAFETY: Caller is expected to only reload timer when appropriate.
pub unsafe fn reload_timer(ms_multiplier: core::num::NonZeroU32) {
    liblz::structures::apic::set_timer_initial_count(
        ms_multiplier.get() * local_state().local_timer_per_ms,
    );
}

pub fn syscall_stack() -> &'static [u8] {
    &local_state().syscall_stack
}

pub fn privilege_stack() -> &'static [u8] {
    &local_state().privilege_stack
}
