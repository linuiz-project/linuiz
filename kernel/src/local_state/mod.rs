mod int_ctrl;

use bit_field::BitField;
pub use int_ctrl::*;

use crate::clock::AtomicClock;
use core::sync::atomic::AtomicUsize;
use libstd::registers::MSR;

const MAGIC: u64 = 0xF;
pub static INIT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
    assert_eq!(
        MSR::IA32_FS_BASE.read(),
        0,
        "IA32_FS MSR has already been configured."
    );

    let apic_id = {
        libstd::instructions::cpuid::exec(0x1, 0x0)
            .unwrap()
            .ebx()
            .get_bits(24..) as u64
    };
    debug!("Configuring LPU state: {}.", apic_id);

    unsafe {
        let lpu_ptr = alloc::alloc::alloc_zeroed(
            core::alloc::Layout::from_size_align(core::mem::size_of::<LocalState>(), 0x1000)
                .unwrap(),
        ) as *mut LocalState;

        // Write the LPU structure into memory.
        lpu_ptr.write_volatile(LocalState {
            clock: AtomicClock::new(),
            int_ctrl: InterruptController::create(),
        });

        // Convert ptr to 64 bit representation, and write metadata into low bits.
        MSR::IA32_FS_BASE.write(
            *(lpu_ptr as u64)
                .set_bits(0..4, MAGIC)
                .set_bits(4..12, apic_id),
        );
        trace!("IA32_FS updated: 0x{:X}.", MSR::IA32_FS_BASE.read());
        INIT_COUNT.fetch_add(1, core::sync::atomic::Ordering::Relaxed);

        int_ctrl().unwrap().sw_enable();
    }
}

pub fn is_bsp() -> bool {
    MSR::IA32_APIC_BASE.read().get_bit(8)
}

struct LocalState {
    clock: AtomicClock,
    int_ctrl: InterruptController,
}

fn check_magic_and_read_fs_base() -> Option<u64> {
    let fs_base = MSR::IA32_FS_BASE.read();
    if fs_base.get_bits(0..4) == MAGIC {
        Some(fs_base)
    } else {
        None
    }
}

fn try_get_lpu() -> Option<&'static LocalState> {
    check_magic_and_read_fs_base()
        .and_then(|fs_base| unsafe { ((fs_base & !0xFFF) as *const LocalState).as_ref() })
}

pub fn id() -> Option<u8> {
    check_magic_and_read_fs_base().map(|fs_base| fs_base.get_bits(4..12) as u8)
}

pub fn clock() -> Option<&'static AtomicClock> {
    try_get_lpu().map(|lpu| &lpu.clock)
}

pub fn int_ctrl() -> Option<&'static InterruptController> {
    try_get_lpu().map(|lpu| &lpu.int_ctrl)
}
