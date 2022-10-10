mod scheduler;

use libarch::memory::VmemRegister;
pub use scheduler::*;

pub const SYSCALL_STACK_SIZE: usize = 0x4000;

#[repr(C, align(0x1000))]
pub(crate) struct LocalState {
    syscall_stack_ptr: *const (),
    syscall_stack: [u8; SYSCALL_STACK_SIZE],
    magic: u64,
    core_id: u32,
    scheduler: Scheduler,
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
            ((libarch::x64::registers::msr::IA32_KERNEL_GS_BASE::read()) as *mut LocalState).as_mut()
        }
    }
}

/// Initializes the core-local state structure.
///
/// SAFETY: This function invariantly assumes it will only be called once.
pub unsafe fn init(core_id: u32) {
    trace!("Configuring local state: #{}", core_id);

    // TODO configure RISC-V ACLINT
    // TODO abstract this somehow, so we can call e.g. `libarch::interrupts::configure_controller();`
    #[cfg(target_arch = "x86_64")]
    {
        use libarch::{interrupts::Vector, x64::structures::apic};

        apic::software_reset();
        apic::get_timer().set_vector(Vector::Timer as u8);
        apic::get_error().set_vector(Vector::Error as u8).set_masked(false);
        apic::get_performance().set_vector(Vector::Performance as u8).set_masked(true);
        apic::get_thermal_sensor().set_vector(Vector::Thermal as u8).set_masked(true);
        // LINT0&1 should be configured by the APIC reset.
    }

    let local_state_ptr = alloc::alloc::alloc(core::alloc::Layout::from_size_align_unchecked(
        core::mem::size_of::<LocalState>(),
        core::mem::align_of::<LocalState>(),
    ))
    .cast::<LocalState>();
    assert!(!local_state_ptr.is_null());

    local_state_ptr.write(LocalState {
        syscall_stack_ptr: core::ptr::null(),
        syscall_stack: [0u8; SYSCALL_STACK_SIZE],
        magic: LocalState::MAGIC,
        core_id,
        scheduler: Scheduler::new(
            false,
            crate::time::Timer::new(1000).unwrap(),
            Task::new(
                TaskPriority::new(1).unwrap(),
                TaskStart::Function(libarch::interrupts::wait_loop),
                TaskStack::At(libcommon::Address::<libcommon::Virtual>::from_ptr({
                    alloc::alloc::alloc_zeroed(core::alloc::Layout::from_size_align_unchecked(0x10, 0x10))
                })),
                {
                    #[cfg(target_arch = "x86_64")]
                    {
                        use libarch::x64;

                        (
                            x64::cpu::GeneralContext::empty(),
                            x64::cpu::SpecialContext::with_kernel_segments(x64::registers::RFlags::INTERRUPT_FLAG),
                        )
                    }
                },
                VmemRegister::read(),
            ),
        ),
    });
    // Write out correct syscall stack pointer.
    local_state_ptr.cast::<*const ()>().write(local_state_ptr.cast::<u8>().add(8 + SYSCALL_STACK_SIZE).cast());

    #[cfg(target_arch = "x86_64")]
    libarch::x64::registers::msr::IA32_KERNEL_GS_BASE::write(local_state_ptr as usize as u64);
}

pub fn with_scheduler<T>(func: impl FnOnce(&mut Scheduler) -> T) -> Option<T> {
    libarch::interrupts::without(|| get_local_state().map(|local_state| func(&mut local_state.scheduler)))
}
