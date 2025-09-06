use crate::{
    mem::{
        HigherHalfDirectMap,
        addr::{
            phys::{FrameAddress, StandardFrame},
            virt::StandardPage,
        },
        pmm::PhysicalMemoryManager,
    },
    scheduler::Scheduler,
    time::LocalTimer,
    util::sync::Mutex,
};
use core::{num::NonZero, ptr::NonNull};

#[cfg(target_arch = "x86_64")]
use crate::arch::x86_64::structures::tss::TaskStateSegment;

pub const STACK_SIZE: usize = 0x10000;
pub const SYSCALL_STACK_SIZE: usize = 0x40000;

fn try_get_local_static_ptr() -> Option<NonNull<LocalState>> {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x86_64::registers::model_specific::IA32_KERNEL_GS_BASE::get_local_state_ptr()
    }
}

/// Local (to the current processor) state structure.
pub struct LocalState {
    timer: Mutex<LocalTimer>,
    scheduler: Mutex<Scheduler>,

    #[cfg(target_arch = "x86_64")]
    tss: TaskStateSegment,
}

const_assert!(size_of::<LocalState>() <= StandardFrame::size_in_bytes());
const_assert!(align_of::<LocalState>() <= StandardFrame::size_in_bytes());

impl LocalState {
    /// Initializes the local state structure.
    pub fn init() {
        assert!(
            try_get_local_static_ptr().is_none(),
            "local state has already been initialized"
        );

        trace!("Configuring local timer...");
        let timer = LocalTimer::configure();

        trace!("Configuring local scheduler...");
        let scheduler = Scheduler::new();

        let local_state_frame = PhysicalMemoryManager::next_free_frame::<StandardFrame>(false)
            .expect("failed to allocate space for local state structure");
        let local_state_address =
            HigherHalfDirectMap::frame_to_page::<_, StandardPage>(local_state_frame);
        let local_state_address =
            NonZero::<usize>::try_from(usize::from(local_state_address)).unwrap();
        let mut local_state_ptr = NonNull::<Self>::with_exposed_provenance(local_state_address);

        // Safety: Memory was allocated for the size and align of `Self`.
        unsafe {
            local_state_ptr.write(Self {
                timer: Mutex::new(timer),
                scheduler: Mutex::new(scheduler),

                #[cfg(target_arch = "x86_64")]
                tss: TaskStateSegment::allocate(),
            });
        }

        #[cfg(target_arch = "x86_64")]
        // Safety:
        // - `local_state_ptr` is initialized.
        // - `Self` size & align are const-asserted to be <= `page_size`.
        // - `local_state_ptr` is unaliased, and this aliasing is local to the statement.
        (unsafe { local_state_ptr.as_mut() }).tss.load();

        // Set the local state pointer for this processor.
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::model_specific::IA32_KERNEL_GS_BASE;

                debug_assert!(IA32_KERNEL_GS_BASE::get_local_state_ptr().is_none());

                // Safety: Processor-local state pointer is not in use.
                unsafe {
                    IA32_KERNEL_GS_BASE::set_local_state_ptr(local_state_ptr);
                }
            }

            _ => { unimplemented!() }
        }

        debug!("Local state has been initialized.");
    }

    /// Gets the local processor state structure.
    fn get_local_static() -> &'static Self {
        try_get_local_static_ptr()
            .map(|local_state_ptr| {
                // Safety: If the state pointer is non-null, the kernel guarantees it will be
                // valid for reading as `LocalState`.
                unsafe { local_state_ptr.as_ref() }
            })
            .expect("local state has not been initialized")
    }

    pub fn with_scheduler<T>(func: impl FnOnce(&mut Scheduler) -> T) -> T {
        Self::get_local_static().scheduler.with_lock(func)
    }

    pub fn with_timer<T>(func: impl FnOnce(&mut LocalTimer) -> T) -> T {
        Self::get_local_static().timer.with_lock(func)
    }
}
