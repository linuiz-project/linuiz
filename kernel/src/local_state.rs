use crate::{clock::AtomicClock, println, scheduling::Scheduler};
use core::sync::atomic::{AtomicUsize, Ordering};
use libkernel::{memory::PageManager, Address, Virtual};
use spin::{Mutex, MutexGuard};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    GlobalTimer = 32,
    LocalTimer = 48,
    CMCI = 49,
    Performance = 50,
    ThermalSensor = 51,
    LINT0 = 52,
    LINT1 = 53,
    Error = 54,
    Storage = 55,
    // APIC spurious interrupt is default mapped to 255.
    Spurious = u8::MAX,
}

#[repr(usize)]
pub enum Offset {
    ID = 0x100,
    Clock = 0x110,
    LocalTimerPerMs = 0x120,
    SchedulerPtr = 0x130,
    SyscallStackPtr = 0x140,
    PrivilegeStackPtr = 0x150,
}

#[repr(align(0x1000))]
struct LocalState {
    magic: u64,
    id: u32,
    clock: AtomicClock,
    scheduler: Mutex<Scheduler>,
    local_timer_per_ms: Option<u32>,
    syscall_stack: [u8; 0x4000],
    privilege_stack: [u8; 0x4000],
}

impl LocalState {
    const MAGIC: u64 = 0xFFFF_D3ADC0DE_FFFF;

    fn validate_init(&self) {
        debug_assert!(self.magic == LocalState::MAGIC);
    }
}

const PML4_LOCAL_STATE_ENTRY_INDEX: usize = 510;
static LOCAL_STATE_PTR_BASE: AtomicUsize = AtomicUsize::new(0);

/// Returns the pointer to the local state structure.
///
/// SAFETY: It is important that, prior to utilizing the value returned by
///         this function, it is ensured that the memory it refers to is
///         actually mapped via the virtual memory manager. No guarantees
///         are made as to whether this has been done or not.
#[inline(always)]
unsafe fn get_local_state_ptr() -> *mut LocalState {
    (LOCAL_STATE_PTR_BASE.load(Ordering::Relaxed) as *mut LocalState)
        // TODO move to a core-local PML4 copy with an L4 local state mapping
        .add(libkernel::cpu::get_id() as usize)
}

fn local_state() -> &'static mut LocalState {
    unsafe { get_local_state_ptr().as_mut().unwrap() }
}

/// Initializes the core-local state structure.
///
/// The local state structure is created via the following process:
///     1.  Compute the address of the structure within the `PML4_LOCAL_STATE_ENTRY_INDEX` page index,
///         along with a randomized slide to ensure the structure cannot be arbitrarily accessed.
///
///     2.  The computed address is map the `PML4_LOCAL_STATE_ENTRY_INDEX` page index base to the given address in
///         a fresh PML4 table for the local core. This fresh PML4 is then written to CR3 to begin constructing the
///         local state.
///
///     3.  The local state is constructed with requisite default values.
///
/// It must be noted that at this point, the local state is still not *initialized*, i.e. the local APIC is not functioning
/// (thus no core-local clock).
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn create() {
    LOCAL_STATE_PTR_BASE.compare_exchange(
        0,
        // Cosntruct the local state pointer (with slide) via the `Address` struct, to
        // automatically sign extend.
        Address::<Virtual>::new(
            ((PML4_LOCAL_STATE_ENTRY_INDEX * libkernel::memory::PML4_ENTRY_MEM_SIZE)
                + (libkernel::instructions::rdrand32().unwrap() as usize))
                & !0xFFF,
        )
        .as_usize(),
        Ordering::AcqRel,
        Ordering::Relaxed,
    );

    let local_state_ptr = get_local_state_ptr();

    {
        use libkernel::memory::Page;

        // Map the pages this local state will utilize.
        let page_manager = libkernel::memory::global_pmgr();
        let base_page = Page::from_ptr(local_state_ptr);
        let end_page = Page::from_ptr(local_state_ptr.add(libkernel::align_up_div(
            core::mem::size_of::<LocalState>(),
            0x1000,
        )));
        (base_page..end_page)
            .for_each(|page| page_manager.auto_map(&page, libkernel::memory::PageAttributes::DATA));
    }

    local_state_ptr.write_volatile(LocalState {
        magic: LocalState::MAGIC,
        id: libkernel::cpu::get_id(),
        clock: AtomicClock::new(),
        scheduler: Mutex::new(Scheduler::new()),
        local_timer_per_ms: None,
        syscall_stack: [0u8; 0x4000],
        privilege_stack: [0u8; 0x4000],
    });
}

// pub fn init_local_apic() {
//     use libkernel::structures::apic::*;

//     // Ensure interrupts are enabled.
//     libkernel::instructions::interrupts::enable();

//     trace!("Configuring APIC & APIT.");
//     unsafe {
//         APIC::configure_spurious();
//         APIC::reset();
//     }

//     APIC::write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
//     APIC::timer().set_mode(TimerMode::OneShot);

//     let per_10ms = {
//         //trace!("Determining APIT frequency.");
//         // Wait on the global timer, to ensure we're starting the count on the rising edge of each millisecond.
//         crate::clock::global::busy_wait_msec(1);
//         // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
//         APIC::write_register(Register::TimerInitialCount, u32::MAX);
//         // Wait for 10ms to get good average tickrate.
//         crate::clock::global::busy_wait_msec(10);

//         APIC::read_register(Register::TimerCurrentCount)
//     };

//     let per_ms = (u32::MAX - per_10ms) / 10;
//     unsafe { get_ptr(Offset::LocalTimerPerMs).cast::<u32>().write(per_ms) };
//     trace!("APIT frequency: {}Hz", per_10ms * 100);

//     // Configure timer vector.
//     APIC::timer().set_vector(InterruptVector::LocalTimer as u8);
//     APIC::timer().set_masked(false);
//     // Configure error vector.
//     APIC::err().set_vector(InterruptVector::Error as u8);
//     APIC::err().set_masked(false);
//     // Set default vectors.
//     // REMARK: Any of these left masked are not currently supported.
//     APIC::cmci().set_vector(InterruptVector::CMCI as u8);
//     APIC::performance().set_vector(InterruptVector::Performance as u8);
//     APIC::thermal_sensor().set_vector(InterruptVector::ThermalSensor as u8);
//     APIC::lint0().set_vector(InterruptVector::LINT0 as u8);
//     APIC::lint1().set_vector(InterruptVector::LINT1 as u8);

//     trace!("Core-local APIC configured.");
// }

#[inline]
pub fn id() -> u32 {
    local_state().validate_init();
    local_state().id
}

#[inline]
pub fn clock() -> &'static AtomicClock {
    local_state().validate_init();
    &local_state().clock
}

/// SAFETY: Caller is expected to only reload timer when appropriate.
// pub unsafe fn reload_timer(ms_multiplier: Option<NonZeroU32>) {
//     let per_ms = get_ptr(Offset::LocalTimerPerMs).cast::<u32>().read();

//     assert_ne!(per_ms, 0, "Kernel GS base is likely not swapped in.");

//     APIC::write_register(
//         libkernel::structures::apic::Register::TimerInitialCount,
//         ms_multiplier.unwrap_or(NonZeroU32::new_unchecked(1)).get() * per_ms,
//     );
// }

#[inline]
pub fn lock_scheduler() -> MutexGuard<'static, Scheduler> {
    local_state().validate_init();
    local_state().scheduler.lock()
}

#[inline]
pub fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    local_state().validate_init();
    local_state().scheduler.try_lock()
}

pub fn syscall_stack() -> &'static [u8] {
    local_state().validate_init();
    &local_state().syscall_stack
}

pub fn privilege_stack() -> &'static [u8] {
    local_state().validate_init();
    &local_state().privilege_stack
}
