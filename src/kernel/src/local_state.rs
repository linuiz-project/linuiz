use crate::{
    memory::VmemRegister,
    scheduling::{Scheduler, Task, TaskPriority},
};

pub const SYSCALL_STACK_SIZE: u64 = 0x4000;

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    syscall_stack_ptr: *const (),
    magic: u64,
    syscall_stack: [u8; SYSCALL_STACK_SIZE as usize],
    core_id: u32,
    timer: alloc::boxed::Box<dyn crate::time::timer::Timer>,
    scheduler: Scheduler,
    default_task: Task,
    cur_task: Option<Task>,
}

impl LocalState {
    const MAGIC: u64 = 0x1234_B33F_D3AD_C0DE;

    fn is_valid_magic(&self) -> bool {
        self.magic == LocalState::MAGIC
    }
}

/// Returns the pointer to the local state structure.
#[inline]
fn get_local_state() -> Option<&'static mut LocalState> {
    unsafe {
        #[cfg(target_arch = "x86_64")]
        {
            ((crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::read()) as *mut LocalState).as_mut()
        }
    }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init(core_id: u32) {
    let local_state_ptr =
        crate::memory::allocate_pages(libkernel::align_up_div(core::mem::size_of::<LocalState>(), 0x1000))
            .cast::<LocalState>();

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(local_state_ptr as usize as u64);
    // TODO abstract the `tp` register
    #[cfg(target_arch = "riscv64")]
    core::arch::asm!("mv tp, {}", in(reg) local_state_pages as u64, options(nostack, nomem));

    trace!("Configuring local state: #{}", core_id);

    {
        use libkernel::memory::Page;

        // Map the pages this local state will utilize.
        let frame_manager = crate::memory::get_kernel_frame_manager();
        let page_manager = crate::memory::get_kernel_page_manager();
        let base_page = Page::from_ptr(local_state_ptr).unwrap();
        let end_page = base_page.forward_checked(core::mem::size_of::<LocalState>() / 0x1000).unwrap();
        (base_page..end_page)
            .for_each(|page| page_manager.auto_map(&page, crate::memory::PageAttributes::RW, frame_manager));
    }

    /* CONFIGURE TIMER */
    // TODO configure RISC-V ACLINT
    // TODO abstract this somehow, so we can call e.g. `crate::interrupts::configure_controller();`
    #[cfg(target_arch = "x86_64")]
    {
        use crate::arch::x64::structures::apic;
        use crate::interrupts::Vector;

        trace!("Configuring local APIC...");
        apic::software_reset();
        apic::set_timer_divisor(apic::TimerDivisor::Div1);
        apic::get_timer().set_vector(Vector::Timer as u8).set_masked(false);
        apic::get_error().set_vector(Vector::Error as u8).set_masked(false);
        apic::get_performance().set_vector(Vector::Performance as u8);
        apic::get_thermal_sensor().set_vector(Vector::Thermal as u8);
        // LINT0&1 should be configured by the APIC reset.
    }

    // Ensure interrupts are enabled after APIC is reset.
    crate::interrupts::enable();

    trace!("Writing local state struct out to memory.");
    local_state_ptr.write(LocalState {
        syscall_stack_ptr: (local_state_ptr as *const u8)
            // `::syscall_stack_ptr`
            .add(8)
            // `::magic`
            .add(8)
            // `::syscall_stack`
            .add(SYSCALL_STACK_SIZE as usize)
            // now we have a valid stack pointer
            .cast(),
        magic: LocalState::MAGIC,
        syscall_stack: [0u8; SYSCALL_STACK_SIZE as usize],
        core_id,
        timer: crate::time::timer::configure_new_timer(1000),
        scheduler: Scheduler::new(false),
        default_task: Task::new(
            TaskPriority::new(1).unwrap(),
            crate::scheduling::TaskStart::Function(crate::interrupts::wait_loop),
            crate::scheduling::TaskStack::None,
            {
                #[cfg(target_arch = "x86_64")]
                {
                    use crate::arch::x64;

                    (
                        x64::cpu::GeneralContext::empty(),
                        x64::cpu::SpecialContext::with_kernel_segments(x64::registers::RFlags::INTERRUPT_FLAG),
                    )
                }
            },
            VmemRegister::read(),
        ),
        cur_task: None,
    });

    match get_local_state() {
        Some(local_state) if local_state.is_valid_magic() => {}
        _ => panic!("local state is invalid after write"),
    }

    trace!("Local state structure written to memory and validated.");
}

/// Attempts to schedule the next task in the local task queue.
pub fn schedule_next_task(
    ctrl_flow_context: &mut crate::interrupts::ControlFlowContext,
    arch_context: &mut crate::interrupts::ArchContext,
) {
    const MIN_TIME_SLICE_MS: u16 = 1;
    const PRIO_TIME_SLICE_MS: u16 = 2;

    let local_state = get_local_state().expect("local state is uninitialized");

    // Move the current task, if any, back into the scheduler queue.
    if let Some(mut cur_task) = local_state.cur_task.take() {
        cur_task.ctrl_flow_context = *ctrl_flow_context;
        cur_task.arch_context = *arch_context;
        cur_task.root_page_table_args = VmemRegister::read();

        local_state.scheduler.push_task(cur_task);
    }

    if let Some(mut global_tasks) = crate::scheduling::GLOBAL_TASKS.try_lock()
        && let Some(task) = global_tasks.pop_front() {
            local_state.scheduler.push_task(task);
    }

    unsafe {
        let next_timer_ms = if let Some(next_task) = local_state.scheduler.pop_task() {
            // Modify interrupt contexts (usually, the registers).
            *ctrl_flow_context = next_task.ctrl_flow_context;
            *arch_context = next_task.arch_context;

            // Set current page tables.
            VmemRegister::write(&next_task.root_page_table_args);

            let next_timer_ms = (next_task.priority().get() as u16) * PRIO_TIME_SLICE_MS;
            local_state.cur_task = Some(next_task);

            next_timer_ms
        } else {
            let default_task = &local_state.default_task;

            // Modify interrupt contexts (usually, the registers).
            *ctrl_flow_context = default_task.ctrl_flow_context;
            *arch_context = default_task.arch_context;

            // Set current page tables.
            VmemRegister::write(&default_task.root_page_table_args);

            MIN_TIME_SLICE_MS
        };

        reload_timer(core::num::NonZeroU16::new(next_timer_ms).unwrap());
    }
}

/// Reloads the local APIC timer with the given millisecond multiplier.
///
/// SAFETY: Caller is expected to only reload timer when appropriate.
unsafe fn reload_timer(freq_multiplier: core::num::NonZeroU16) {
    get_local_state()
        .expect("reload timer called for uninitialized local state")
        .timer
        .set_next_wait(freq_multiplier.get());
}

/// Attempts to begin scheduling tasks on the current thread. If the scheduler has already been
/// enabled, or local state has not been initialized, this function does nothing.
pub fn try_begin_scheduling() {
    if let Some(local_state) = get_local_state() {
        let scheduler = &mut local_state.scheduler;

        if !scheduler.is_enabled() {
            trace!("Enabling kernel scheduler.");
            scheduler.enable();

            unsafe { reload_timer(core::num::NonZeroU16::new_unchecked(1)) };
        }
    }
}

/// Attempts to push a task to the core-local scheduler directly. If the core-local state is not
/// initialized, then the task is returned as an `Err(Task)`.
pub fn try_push_task(task: Task) -> Result<(), Task> {
    match get_local_state() {
        Some(local_state) => {
            local_state.scheduler.push_task(task);
            Ok(())
        }
        None => Err(task),
    }
}
