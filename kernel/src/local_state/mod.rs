mod timer;

use crate::scheduling::{Scheduler, Task, TaskPriority};
use core::sync::atomic::{AtomicUsize, Ordering};
use libkernel::{
    registers::{control::CR3, RFlags},
    Address, Virtual,
};

static ACTIVE_CPUS: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);

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
    timer: alloc::boxed::Box<dyn timer::Timer>,
    scheduler: Scheduler,
    default_task: Task,
    cur_task: Option<Task>,
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
                    + (libkernel::rand(0..(u32::MAX as u64)).unwrap_or(0) as usize))
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
        let frame_manager = crate::memory::get_kernel_frame_manager().unwrap();
        let page_manager = crate::memory::get_kernel_page_manager().unwrap();
        let base_page = Page::from_ptr(local_state_ptr);
        let end_page = base_page
            .forward_checked(core::mem::size_of::<LocalState>() / 0x1000)
            .unwrap();
        (base_page..end_page).for_each(|page| {
            page_manager.auto_map(
                &page,
                libkernel::memory::PageAttributes::DATA,
                frame_manager,
            )
        });
    }

    /* CONFIGURE APIC */
    use crate::interrupts::Vector;
    use libkernel::structures::apic;

    apic::software_reset();
    apic::set_timer_divisor(apic::TimerDivisor::Div1);
    apic::get_error()
        .set_vector(Vector::Error as u8)
        .set_masked(false);
    apic::get_performance().set_vector(Vector::Performance as u8);
    apic::get_thermal_sensor().set_vector(Vector::ThermalSensor as u8);
    // LINT0&1 should be configured by the APIC reset.

    // Ensure interrupts are completely enabled after APIC is reset.
    libkernel::registers::msr::IA32_APIC_BASE::set_hw_enable(true);
    libkernel::instructions::interrupts::enable();
    libkernel::structures::apic::sw_enable();

    crate::interrupts::set_handler_fn(Vector::LocalTimer, local_timer_handler);
    let mut timer = timer::get_best_timer();
    timer.set_frequency(1000);

    local_state_ptr.write(LocalState {
        privilege_stack: [0u8; 0x4000],
        db_stack: [0u8; 0x1000],
        nmi_stack: [0u8; 0x1000],
        df_stack: [0u8; 0x1000],
        mc_stack: [0u8; 0x1000],
        magic: LocalState::MAGIC,
        timer,
        scheduler: Scheduler::new(false),
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
    });
    local_state().unwrap().validate_init();

    ACTIVE_CPUS.fetch_add(1, Ordering::Relaxed);
}

fn local_timer_handler(
    stack_frame: &mut x86_64::structures::idt::InterruptStackFrame,
    cached_regs: &mut crate::scheduling::ThreadRegisters,
) {
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

        local_state.scheduler.push_task(cur_task);
    }

    // Take all tasks from the global queue.
    while let Some(task) = unsafe { crate::scheduling::GLOBAL_TASK_QUEUE.pop() } {
        local_state.scheduler.push_task(task);
    }

    {
        let page_manager = crate::memory::get_kernel_page_manager().unwrap();

        for local_state_index in 0..ACTIVE_CPUS.load(Ordering::Relaxed) {
            let other_ptr = unsafe {
                (LOCAL_STATES_BASE.load(Ordering::Relaxed) as *mut LocalState)
                    .add(local_state_index)
            };

            if !page_manager.is_mapped(Address::<Virtual>::from_ptr(other_ptr)) {
                continue;
            }

            let other = unsafe { other_ptr.as_mut().unwrap() };
            let other_avg_prio = other.scheduler.get_avg_prio();
            let self_avg_prio = local_state.scheduler.get_avg_prio();
            let avg_prio_diff = self_avg_prio.abs_diff(other_avg_prio);
        }
    }

    // load balance tasks
    // {
    //     let rand_index = libkernel::rand(0..ACTIVE_CPUS.load(Ordering::Relaxed)).expect(
    //         "hardware random number generation must be supported for load-balanced scheduling",
    //     ) as usize;
    //     crate::print!(
    //         "rand {:?} {}",
    //         0..ACTIVE_CPUS.load(Ordering::Relaxed),
    //         rand_index
    //     );

    //     let other_ptr = unsafe {
    //         (LOCAL_STATES_BASE.load(Ordering::Relaxed) as *mut LocalState).add(rand_index)
    //     };

    //     if crate::memory::get_kernel_page_manager()
    //         .unwrap()
    //         .is_mapped(Address::<Virtual>::from_ptr(other_ptr))
    //     {
    //         crate::print!("mapped");

    //         let other = unsafe { other_ptr.as_mut().unwrap() };

    //         let self_avg_prio = local_state.scheduler.get_avg_prio();
    //         let other_avg_prio = other.scheduler.get_avg_prio();
    //         const MAX_PRIO_DIFF: u64 = (TaskPriority::MAX + TaskPriority::MIN) as u64;

    //         if self_avg_prio.abs_diff(other_avg_prio) >= MAX_PRIO_DIFF {
    //             while self_avg_prio > other_avg_prio {
    //                 other.scheduler.push_task(
    //                     local_state
    //                         .scheduler
    //                         .pop_task()
    //                         .expect("local scheduler failed to pop task for load balancing"),
    //                 );
    //             }

    //             while self_avg_prio < other_avg_prio {
    //                 local_state.scheduler.push_task(
    //                     other
    //                         .scheduler
    //                         .pop_task()
    //                         .expect("other scheduler failed to pop task for load balancing"),
    //                 );
    //             }
    //         }
    //     }
    // }

    unsafe {
        let next_timer_ms = if let Some(next_task) = local_state.scheduler.pop_task() {
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
    local_state()
        .expect("reload timer called for uninitialized local state")
        .timer
        .reload(ms_multiplier.get());
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

pub unsafe fn get_scheduler() -> Option<&'static Scheduler> {
    local_state().map(|local_state| &local_state.scheduler)
}
