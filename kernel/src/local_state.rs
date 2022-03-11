use crate::{
    clock::{local, AtomicClock},
    scheduling::Scheduler,
};
use core::{
    arch::asm,
    num::{NonZeroU32, NonZeroUsize},
    sync::atomic::{AtomicU32, AtomicU64, AtomicUsize, Ordering},
};
use libkernel::{
    memory::PageManager,
    registers::msr::{Generic, IA32_KERNEL_GS_BASE},
    structures::apic::APIC,
    Address, Virtual,
};
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
    page_manager: PageManager,
    id: u32,
    clock: AtomicClock,
    scheduler: Mutex<Scheduler>,
    syscall_stack_ptr: *const (),
    privilege_stack_ptr: *const (),
    local_timer_per_ms: Option<u32>,
}

impl LocalState {
    const MAGIC: u64 = 0xFFFF_D3ADC0DE_FFFF;

    fn validate_init(&self) {
        debug_assert!(self.magic == LocalState::MAGIC);
    }
}

const PML4_LOCAL_STATE_ENTRY_INDEX: usize = 510;
static LOCAL_STATE_PTR: AtomicUsize = AtomicUsize::new(0);

fn local_state() -> &'static mut LocalState {
    unsafe {
        (LOCAL_STATE_PTR.load(Ordering::Relaxed) as *mut LocalState)
            .as_mut()
            .unwrap()
    }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init() {
    LOCAL_STATE_PTR.compare_exchange(
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

    let local_state_ptr = LOCAL_STATE_PTR.load(Ordering::Relaxed) as *mut LocalState;

    let page_manager = {
        let global_page_manager = libkernel::memory::global_pgmr();
        let mut copy_pml4 = global_page_manager.copy_pml4();
        copy_pml4
            .get_entry_mut(PML4_LOCAL_STATE_ENTRY_INDEX)
            .clear();
        let page_manager = libkernel::memory::PageManager::new(
            &global_page_manager.mapped_page(),
            Some(copy_pml4),
        );

        page_manager.auto_map(&libkernel::memory::Page::from_ptr(local_state_ptr), {
            use libkernel::memory::PageAttributes;
            PageAttributes::PRESENT | PageAttributes::WRITABLE | PageAttributes::NO_EXECUTE
        });
        page_manager.write_cr3();

        page_manager
    };

    local_state_ptr.write(LocalState {
        magic: LocalState::MAGIC,
        page_manager,
        id: libkernel::cpu::get_id(),
        clock: AtomicClock::new(),
        scheduler: Mutex::new(Scheduler::new()),
        syscall_stack_ptr: 0x0 as *const (),   // TODO
        privilege_stack_ptr: 0x0 as *const (), // TODO
        local_timer_per_ms: None,
    });

    // ptr.add(Offset::SyscallStackPtr as usize)
    //     .cast::<*const u8>()
    //     .write(alloc_stack(2));
    // ptr.add(Offset::PrivilegeStackPtr as usize)
    //     .cast::<*const u8>()
    //     .write(alloc_stack(2));
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
    unsafe {
        local_state().validate_init();
        local_state().id
    }
}

#[inline]
pub fn clock() -> &'static AtomicClock {
    unsafe {
        local_state().validate_init();
        &local_state().clock
    }
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
    unsafe {
        local_state().validate_init();
        local_state().scheduler.lock()
    }
}

#[inline]
pub fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    unsafe {
        local_state().validate_init();
        local_state().scheduler.try_lock()
    }
}
