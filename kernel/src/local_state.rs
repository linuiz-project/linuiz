use crate::{
    clock::{local, AtomicClock},
    scheduling::Scheduler,
};
use core::{
    arch::asm,
    num::{NonZeroU32, NonZeroUsize},
    sync::atomic::{AtomicU32, AtomicU64},
};
use libkernel::{
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

const BASE_ADDR: Address<Virtual> = unsafe { Address::<Virtual>::new_unsafe(0x808000000000) };
static BASE_ADDR_SLIDE: AtomicU64 = AtomicU64::new(0);

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init() {
    let ptr_slide = BASE_ADDR_SLIDE
        .compare_exchange(
            0,
            {
                let mut rdrand = libkernel::instructions::rdrand().unwrap_or(0)
                    // Page-align the random offset.
                    & !0xFFF;

                while rdrand > libkernel::PT_L4_ENTRY_MEM {
                    rdrand /= 2;
                }

                rdrand
            },
            core::sync::atomic::Ordering::AcqRel,
            core::sync::atomic::Ordering::Acquire,
        )
        .unwrap_unchecked();

    let ptr = (BASE_ADDR + (ptr_slide as usize)).as_mut_ptr::<u8>();

    ptr.cast::<libkernel::memory::PageManager>().write({
        let global_page_manager = libkernel::memory::global_pgmr();
        let page_manager = libkernel::memory::PageManager::new(
            &global_page_manager.mapped_page(),
            Some(global_page_manager.copy_pml4()),
        );

        page_manager.auto_map(&libkernel::memory::Page::from_ptr(ptr), {
            use libkernel::memory::PageAttributes;
            PageAttributes::PRESENT | PageAttributes::WRITABLE | PageAttributes::NO_EXECUTE
        });
        page_manager.write_cr3();

        core::ptr::write_bytes(ptr, 0x0, 0x1000);

        page_manager
    });
    ptr.cast::<*const u8>().write(ptr);
    ptr.add(Offset::ID as usize)
        .cast::<u32>()
        .write(libkernel::cpu::get_id());
    ptr.add(Offset::Clock as usize)
        .cast::<AtomicClock>()
        .write(AtomicClock::new());
    ptr.add(Offset::SchedulerPtr as usize)
        .cast::<*const Mutex<Scheduler>>()
        .write({
            use alloc::boxed::Box;
            Box::leak(Box::new(Mutex::new(Scheduler::new()))) as *mut _
        });
    // ptr.add(Offset::SyscallStackPtr as usize)
    //     .cast::<*const u8>()
    //     .write(alloc_stack(2));
    // ptr.add(Offset::PrivilegeStackPtr as usize)
    //     .cast::<*const u8>()
    //     .write(alloc_stack(2));
}

pub fn init_local_apic() {
    use libkernel::structures::apic::*;

    // Ensure interrupts are enabled.
    libkernel::instructions::interrupts::enable();

    trace!("Configuring APIC & APIT.");
    unsafe {
        APIC::configure_spurious();
        APIC::reset();
    }

    APIC::write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
    APIC::timer().set_mode(TimerMode::OneShot);

    let per_10ms = {
        //trace!("Determining APIT frequency.");
        // Wait on the global timer, to ensure we're starting the count on the rising edge of each millisecond.
        crate::clock::global::busy_wait_msec(1);
        // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
        APIC::write_register(Register::TimerInitialCount, u32::MAX);
        // Wait for 10ms to get good average tickrate.
        crate::clock::global::busy_wait_msec(10);

        APIC::read_register(Register::TimerCurrentCount)
    };

    let per_ms = (u32::MAX - per_10ms) / 10;
    unsafe { get_ptr(Offset::LocalTimerPerMs).cast::<u32>().write(per_ms) };
    trace!("APIT frequency: {}Hz", per_10ms * 100);

    // Configure timer vector.
    APIC::timer().set_vector(InterruptVector::LocalTimer as u8);
    APIC::timer().set_masked(false);
    // Configure error vector.
    APIC::err().set_vector(InterruptVector::Error as u8);
    APIC::err().set_masked(false);
    // Set default vectors.
    // REMARK: Any of these left masked are not currently supported.
    APIC::cmci().set_vector(InterruptVector::CMCI as u8);
    APIC::performance().set_vector(InterruptVector::Performance as u8);
    APIC::thermal_sensor().set_vector(InterruptVector::ThermalSensor as u8);
    APIC::lint0().set_vector(InterruptVector::LINT0 as u8);
    APIC::lint1().set_vector(InterruptVector::LINT1 as u8);

    trace!("Core-local APIC configured.");
}

fn get_ptr(offset: Offset) -> *mut () {
    unsafe {
        (BASE_ADDR
            + ((BASE_ADDR_SLIDE.load(core::sync::atomic::Ordering::Acquire) + (offset as u64))
                as usize))
            .as_mut_ptr::<()>()
    }
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn id() -> u32 {
    get_ptr(Offset::ID).cast::<u32>().read()
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn clock() -> &'static AtomicClock {
    get_ptr(Offset::Clock)
        .cast::<AtomicClock>()
        .as_ref()
        .unwrap()
}

/// SAFETY: Caller is expected to only reload timer when appropriate.
pub unsafe fn reload_timer(ms_multiplier: Option<NonZeroU32>) {
    let per_ms = get_ptr(Offset::LocalTimerPerMs).cast::<u32>().read();

    assert_ne!(per_ms, 0, "Kernel GS base is likely not swapped in.");

    APIC::write_register(
        libkernel::structures::apic::Register::TimerInitialCount,
        ms_multiplier.unwrap_or(NonZeroU32::new_unchecked(1)).get() * per_ms,
    );
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get_ptr(Offset::SchedulerPtr)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .map(|scheduler| scheduler.lock())
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    get_ptr(Offset::SchedulerPtr)
        .cast::<Mutex<Scheduler>>()
        .as_ref()
        .and_then(|scheduler| scheduler.try_lock())
}
