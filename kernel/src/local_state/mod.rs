mod int_ctrl;

use bit_field::BitField;
pub use int_ctrl::*;
use spin::{Mutex, MutexGuard};

use crate::{clock::AtomicClock, scheduling::Scheduler};
use core::{ops::Range, sync::atomic::AtomicUsize};
use lib::registers::{msr, msr::Generic};

pub static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn is_bsp() -> bool {
    msr::IA32_APIC_BASE::is_bsp()
}

struct LocalStateRegister;

/// Layout of LocalStateRegister:
/// Bit 0       ID FLAG
/// Bit 1..10   ID BITS
/// Bit 10..13  RESERVED
/// Bit 13..64  STRUCTURE PTR
impl LocalStateRegister {
    const ID_SET_BIT: usize = 0;
    const ID_BITS: Range<usize> = 1..10;
    const DATA_BITS: Range<usize> = 0..12;
    const PTR_BITS: Range<usize> = 12..64;

    #[inline]
    fn get_id() -> u8 {
        let gs_base = msr::IA32_GS_BASE::read();

        if !gs_base.get_bit(Self::ID_SET_BIT) {
            let cpuid_id = (lib::instructions::cpuid::exec(0x1, 0x0).unwrap().ebx() >> 24) as u64;

            unsafe {
                msr::IA32_GS_BASE::write(
                    *msr::IA32_GS_BASE::read().set_bits(Self::ID_BITS, cpuid_id),
                )
            };
        }

        gs_base.get_bits(Self::ID_BITS) as u8
    }

    fn try_get() -> Option<&'static LocalState> {
        unsafe {
            ((*msr::IA32_GS_BASE::read().set_bits(Self::DATA_BITS, 0)) as *const LocalState)
                .as_ref()
        }
    }

    fn set_ptr(ptr: *mut LocalState) {
        let ptr_u64 = ptr as u64;

        assert_eq!(
            ptr_u64.get_bits(Self::DATA_BITS),
            0,
            "Local state pointer must be page-aligned (low 12 bits are data bits)."
        );

        unsafe {
            msr::IA32_GS_BASE::write(ptr_u64 | msr::IA32_GS_BASE::read().get_bits(Self::DATA_BITS));
        };
    }
}

struct LocalState {
    clock: AtomicClock,
    int_ctrl: InterruptController,
    thread: Mutex<Scheduler>,
}

pub fn init() {
    assert!(
        !LocalStateRegister::try_get().is_some(),
        "Local state register has already been configured."
    );

    INIT_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

    let cpuid_id = {
        lib::instructions::cpuid::exec(0x1, 0x0)
            .unwrap()
            .ebx()
            .get_bits(24..) as u8
    };

    trace!("Configuring local state: {}.", cpuid_id);
    unsafe {
        let lpu_ptr = lib::memory::malloc::get()
            .alloc(
                core::mem::size_of::<LocalState>(),
                // Local state register must be page-aligned.
                core::num::NonZeroUsize::new(0x1000),
            )
            .unwrap()
            .cast::<LocalState>()
            .unwrap()
            .into_parts()
            .0;

        debug!("CPU local state pointer: {:?}", lpu_ptr);

        {
            let clock = AtomicClock::new();
            let int_ctrl = InterruptController::create();
            let thread = Mutex::new(Scheduler::new());

            assert_eq!(
                cpuid_id,
                int_ctrl.apic_id(),
                "CPUID processor ID does not match APIC ID."
            );

            // Write the LPU structure into memory.
            lpu_ptr.write(LocalState {
                clock,
                int_ctrl,
                thread,
            });
        }

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        LocalStateRegister::set_ptr(lpu_ptr);
        int_ctrl().sw_enable();
        int_ctrl().reload_timer(core::num::NonZeroU32::new(1));
    }
}

static LOCAL_STATE_NO_INIT: &str = "Processor local state has not been initialized";

pub fn processor_id() -> u8 {
    LocalStateRegister::get_id()
}

pub fn clock() -> &'static AtomicClock {
    LocalStateRegister::try_get()
        .map(|ls| &ls.clock)
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn int_ctrl() -> &'static InterruptController {
    LocalStateRegister::try_get()
        .map(|ls| &ls.int_ctrl)
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn lock_scheduler() -> MutexGuard<'static, Scheduler> {
    LocalStateRegister::try_get()
        .map(|ls| ls.thread.lock())
        .expect(LOCAL_STATE_NO_INIT)
}

pub fn try_lock_scheduler() -> Option<MutexGuard<'static, Scheduler>> {
    LocalStateRegister::try_get().and_then(|ls| ls.thread.try_lock())
}
