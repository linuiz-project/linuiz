use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::{
    arch::asm,
    num::{NonZeroU32, NonZeroUsize},
};
use libkernel::{
    registers::msr::{Generic, IA32_KERNEL_GS_BASE},
    structures::apic::APIC,
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
    SelfPtr = 0x0,
    ID = 0x10,
    Clock = 0x20,
    LocalTimerPerMs = 0x30,
    SchedulerPtr = 0x40,
    SyscallStackPtr = 0x50,
    PrivilegeStackPtr = 0x60,
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init() {
    assert_eq!(
        IA32_KERNEL_GS_BASE::read(),
        0,
        "Local state register has already been configured."
    );

    let (ptr, size) = libkernel::memory::malloc::get()
        .alloc(
            0x1000,
            // Local state register must be page-aligned.
            NonZeroUsize::new(0x1000),
        )
        .unwrap()
        .into_parts();

    core::ptr::write_bytes(ptr, 0x0, size);

    let alloc_stack = |page_count| {
        let (ptr, len) = libkernel::memory::malloc::get()
            .alloc_pages(page_count)
            .unwrap()
            .1
            .into_parts();

        ptr.add(len)
    };

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
            let scheduler_ptr = libkernel::alloc_obj!(Mutex<Scheduler>);
            scheduler_ptr.write(Mutex::new(Scheduler::new()));
            scheduler_ptr
        });
    ptr.add(Offset::SyscallStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(2));
    ptr.add(Offset::PrivilegeStackPtr as usize)
        .cast::<*const u8>()
        .write(alloc_stack(2));

    // Convert ptr to 64 bit representation, and write metadata into low bits.
    IA32_KERNEL_GS_BASE::write(ptr as u64);
}

/// Enables the core-local state structure.
#[inline]
pub unsafe fn enable() {
    // The `enable()` and `disable()` currently do the exact same thing, but
    // there may be some validation done in the future, to ensure proper usage
    // of `swapgs`.
    x86_64::registers::segmentation::GS::swap()
}

/// Disables the core-local state structure.
#[inline]
pub unsafe fn disable() {
    x86_64::registers::segmentation::GS::swap()
}

#[macro_export]
macro_rules! rdgsval {
    ($ty:ty, $offset:expr) => {
        {
            let val: $ty;

            core::arch::asm!(
                "mov {}, gs:{}",
                out(reg) val,
                const $offset as usize,
                options(nostack, nomem, preserves_flags)
            );

            val
        }
    };

    ($ty:ty, $offset:expr, $reg_size:ident) => {
        {
            let val: $ty;

            core::arch::asm!(
                concat!("mov {:", stringify!($reg_size), "}, gs:{}"),
                out(reg) val,
                const $offset as usize,
                options(nostack, nomem, preserves_flags)
            );

            val
        }
    };
}

#[macro_export]
macro_rules! wrgsval {
    ($offset:expr, $val:expr) => {
        core::arch::asm!(
            "mov gs:{}, {}",
            const $offset as usize,
            in(reg) $val,
            options(nostack, nomem, preserves_flags)
        );
    };
}

#[macro_export]
macro_rules! gs_ptr {
    ($ptr_ty:ty, $offset:expr) => {
        {
            let ptr: *const $ptr_ty;

            asm!(
                "mov {0}, gs:{1}",
                "add {0}, {2}",
                out(reg) ptr,
                const Offset::SelfPtr as usize,
                const $offset as usize,
                options(nostack, nomem)
            );

            ptr
        }
    };
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
    unsafe {
        wrgsval!(Offset::LocalTimerPerMs, per_ms as u64);
    }
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

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn id() -> u32 {
    rdgsval!(u32, Offset::ID, e)
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn clock() -> &'static AtomicClock {
    &*(gs_ptr!(AtomicClock, Offset::Clock))
}

/// SAFETY: Caller is expected to only reload timer when appropriate.
pub unsafe fn reload_timer(ms_multiplier: Option<NonZeroU32>) {
    let per_ms = rdgsval!(u64, Offset::LocalTimerPerMs) as u32;

    assert_ne!(per_ms, 0, "Kernel GS base is likely not swapped in.");

    APIC::write_register(
        libkernel::structures::apic::Register::TimerInitialCount,
        ms_multiplier.unwrap_or(NonZeroU32::new_unchecked(1)).get() * per_ms,
    );
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    rdgsval!(*const Mutex<Scheduler>, Offset::SchedulerPtr)
        .as_ref()
        .map(|scheduler| scheduler.lock())
}

/// SAFETY: Caller must ensure kernel `gs` base is swapped in.
#[inline]
pub unsafe fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    rdgsval!(*const Mutex<Scheduler>, Offset::SchedulerPtr)
        .as_ref()
        .and_then(|scheduler| scheduler.try_lock())
}
