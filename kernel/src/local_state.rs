use crate::scheduling::{Task, TaskPriority};
use core::sync::atomic::{AtomicUsize, Ordering};
use libkernel::{
    registers::{control::CR3, RFlags},
    Address, Virtual,
};

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    // Stacks must go at the beginning of the structure to
    // ensure their alignment is proper.
    //
    // Additionally, stack sizes must have the low 4 bits clear to
    // ensure the next stack's alignment is proper.
    privilege_stack: [u8; 0x4000],
    db_stack: [u8; 0x1000],
    nmi_stack: [u8; 0x1000],
    df_stack: [u8; 0x1000],
    mc_stack: [u8; 0x1000],
    magic: u32,
    default_task: Task,
    cur_task: Option<Task>,
    local_timer_per_ms: u32,
}

impl LocalState {
    const MAGIC: u32 = 0xD3ADC0DE;

    fn validate_init(&self) {
        assert!(self.magic == LocalState::MAGIC);
    }
}

static LOCAL_STATES_BASE: AtomicUsize = AtomicUsize::new(0);

/// Returns the pointer to the local state structure.
///
/// SAFETY: It is important that, prior to utilizing the value returned by
///         this function, it is ensured that the memory it refers to is
///         actually mapped via the virtual memory manager. No guarantees
///         are made as to whether this has been done or not.
#[inline(always)]
unsafe fn get_local_state_ptr() -> *mut LocalState {
    (LOCAL_STATES_BASE.load(Ordering::Relaxed) as *mut LocalState)
        // TODO move to a core-local PML4 copy with an L4 local state mapping ?? or maybe not
        .add(libkernel::cpu::get_id() as usize)
}

#[inline]
fn local_state() -> Option<&'static mut LocalState> {
    unsafe { get_local_state_ptr().as_mut() }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init() {
    LOCAL_STATES_BASE
        .compare_exchange(
            0,
            // Cosntruct the local state pointer (with slide) via the `Address` struct, to
            // automatically sign extend.
            Address::<Virtual>::new(
                ((510 * libkernel::memory::PML4_ENTRY_MEM_SIZE)
                    + (libkernel::instructions::rdrand32().unwrap() as usize))
                    & !0xFFF,
            )
            .as_usize(),
            Ordering::AcqRel,
            Ordering::Acquire,
        )
        .ok();

    let local_state_ptr = get_local_state_ptr();
    {
        use libkernel::memory::Page;

        // Map the pages this local state will utilize.
        let page_manager = libkernel::memory::global_pmgr();
        let base_page = Page::from_ptr(local_state_ptr);
        let end_page = base_page
            .forward_checked(core::mem::size_of::<LocalState>() / 0x1000)
            .unwrap();
        (base_page..end_page)
            .for_each(|page| page_manager.auto_map(&page, libkernel::memory::PageAttributes::DATA));
    }

    /* CONFIGURE APIC */
    use crate::interrupts::Vector;
    use libkernel::structures::apic;

    apic::software_reset();
    apic::set_timer_divisor(apic::TimerDivisor::Div1);
    apic::get_timer()
        .set_mode(apic::TimerMode::OneShot)
        .set_vector(Vector::LocalTimer as u8);
    apic::get_error()
        .set_vector(Vector::Error as u8)
        .set_masked(false);
    crate::interrupts::set_handler_fn(Vector::LocalTimer, local_timer_handler);
    apic::get_performance().set_vector(Vector::Performance as u8);
    apic::get_thermal_sensor().set_vector(Vector::ThermalSensor as u8);
    // LINT0&1 should be configured by the APIC reset.

    // Ensure interrupts are completely enabled after APIC is reset.
    libkernel::registers::msr::IA32_APIC_BASE::set_hw_enable(true);
    libkernel::instructions::interrupts::enable();
    libkernel::structures::apic::sw_enable();

    let per_ms = {
        if let Some(registers) = libkernel::instructions::cpuid::exec(0x15, 0x0)
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
            crate::clock::busy_wait_msec(1);
            apic::set_timer_initial_count(u32::MAX);
            crate::clock::busy_wait_msec(MS_WINDOW as u64);

            let per_ms = (u32::MAX - apic::get_timer_current_count()) / MS_WINDOW;
            trace!(
                "CPU clock frequency measurement: {} Hz",
                per_ms * (1000 / MS_WINDOW)
            );
            per_ms
        }
    };

    local_state_ptr.write(LocalState {
        privilege_stack: [0u8; 0x4000],
        db_stack: [0u8; 0x1000],
        nmi_stack: [0u8; 0x1000],
        df_stack: [0u8; 0x1000],
        mc_stack: [0u8; 0x1000],
        magic: LocalState::MAGIC,
        default_task: Task::new(
            TaskPriority::new(1).unwrap(),
            libkernel::instructions::hlt_indefinite,
            crate::scheduling::TaskStackOption::AutoAllocate,
            RFlags::INTERRUPT_FLAG,
            *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
            *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
            CR3::read(),
        ),
        cur_task: None,
        local_timer_per_ms: per_ms,
    });
    local_state().unwrap().validate_init();
}

fn local_timer_handler(
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    cached_regs: &mut crate::scheduling::ThreadRegisters,
) {
    use crate::scheduling::SCHEDULER;

    const MIN_TIME_SLICE_MS: u32 = 1;
    const PRIO_TIME_SLICE_MS: u32 = 2;

    let local_state =
        local_state().expect("local timer handler called before local state initialization");

    if let Some(mut cur_task) = local_state.cur_task.take() {
        cur_task.rip = stack_frame.instruction_pointer.as_u64();
        cur_task.cs = stack_frame.code_segment as u16;
        cur_task.rsp = stack_frame.stack_pointer.as_u64();
        cur_task.ss = stack_frame.stack_segment as u16;
        cur_task.rfl = unsafe { RFlags::from_bits_unchecked(stack_frame.cpu_flags) };
        cur_task.gprs = *cached_regs;
        cur_task.cr3 = CR3::read();

        SCHEDULER.push_task(cur_task);
    }

    unsafe {
        let next_timer_ms = if let Some(next_task) = crate::scheduling::SCHEDULER.pop_task() {
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
            *cached_regs = next_task.gprs;

            // Set current page tables.
            CR3::write(next_task.cr3.0, next_task.cr3.1);

            let next_timer_ms = (next_task.prio().get() as u32) * PRIO_TIME_SLICE_MS;
            local_state.cur_task = Some(next_task);

            next_timer_ms
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

            MIN_TIME_SLICE_MS
        };

        reload_timer(core::num::NonZeroU32::new(next_timer_ms).unwrap());
        libkernel::structures::apic::end_of_interrupt();
    }
}

/// Reloads the local APIC timer with the given millisecond multiplier.
///
/// SAFETY: Caller is expected to only reload timer when appropriate.
///
/// REMARK: This function will panic if the local state structure is uninitialized.
pub unsafe fn reload_timer(ms_multiplier: core::num::NonZeroU32) {
    libkernel::structures::apic::set_timer_initial_count(
        ms_multiplier.get()
            * local_state()
                .expect("reload timer called for uninitialized local state")
                .local_timer_per_ms,
    );
}

/// Returns a pointer to the top of the privilege stack, or `None` if local state is uninitialized.
pub fn privilege_stack_ptr() -> Option<*const ()> {
    local_state().map(|local_state| local_state.privilege_stack.as_ptr() as *const _)
}

/// Returns a pointer to the top of the #DB stack table, or `None` if local state is uninitialized.
pub fn db_stack_ptr() -> Option<*const ()> {
    local_state().map(|local_state| local_state.db_stack.as_ptr() as *const _)
}

/// Returns a pointer to the top of the #NMI stack, or `None` if local state is uninitialized.
pub fn nmi_stack_ptr() -> Option<*const ()> {
    local_state().map(|local_state| local_state.nmi_stack.as_ptr() as *const _)
}

/// Returns a pointer to the top of the #DF stack, or `None` if local state is uninitialized.
pub fn df_stack_ptr() -> Option<*const ()> {
    local_state().map(|local_state| local_state.df_stack.as_ptr() as *const _)
}

/// Returns a pointer to the top of the #MC stack, or `None` if local state is uninitialized.
pub fn mc_stack_ptr() -> Option<*const ()> {
    local_state().map(|local_state| local_state.mc_stack.as_ptr() as *const _)
}
