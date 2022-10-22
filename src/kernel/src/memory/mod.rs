mod mapper;
mod paging;

pub mod io;
pub mod slab;
pub use mapper::*;
pub use paging::*;

use libcommon::{Address, Frame, Virtual};
use spin::Once;

pub fn get_hhdm_address() -> Address<Virtual> {
    static HHDM_ADDRESS: Once<Address<Virtual>> = Once::new();

    HHDM_ADDRESS
        .call_once(|| {
            static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::boot::LIMINE_REV);

            Address::<Virtual>::new(
                LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset,
            )
            .expect("bootloader provided a non-canonical higher-half direct mapping address")
        })
        .clone()
}

pub fn get_kernel_mapper() -> &'static Mapper {
    static KERNEL_MAPPER: Once<Mapper> = Once::new();

    KERNEL_MAPPER.call_once(|| {
        // SAFETY: The kernel guarantees the HHDM will be valid.
        unsafe { Mapper::new(4, get_hhdm_address(), None).unwrap() }
    })
}

// TODO this
// pub fn reclaim_bootloader_frames() {
//     let frame_manager = get_kernel_frame_manager();
//     frame_manager.iter().enumerate().filter(|(_, (_, ty))| *ty == FrameType::BootReclaim).for_each(
//         |(frame_index, _)| {
//             // SAFETY: These frames come directly from the frame manager, and so are guaranteed valid.
//             let frame = unsafe { Address::<Frame>::new_unchecked((frame_index * 0x1000) as u64) };
//             frame_manager.try_modify_type(frame, FrameType::Usable).ok();
//             frame_manager.free(frame).ok();
//         },
//     );
// }

#[cfg(target_arch = "x86_64")]
pub struct VmemRegister(pub Address<Frame>, pub crate::arch::x64::registers::control::CR3Flags);
#[cfg(target_arch = "riscv64")]
pub struct VmemRegister(pub Address<Frame>, pub u16, pub crate::arch::rv64::registers::satp::Mode);

impl VmemRegister {
    pub fn read() -> Self {
        #[cfg(target_arch = "x86_64")]
        {
            let args = crate::arch::x64::registers::control::CR3::read();
            Self(args.0, args.1)
        }

        #[cfg(target_arch = "riscv64")]
        {
            let args = crate::arch::rv64::registers::satp::read();
            Self(args.0, args.1, args.2)
        }
    }

    /// SAFETY: Writing to this register has the chance to externally invalidate memory references.
    pub unsafe fn write(args: &Self) {
        #[cfg(target_arch = "x86_64")]
        crate::arch::x64::registers::control::CR3::write(args.0, args.1);

        #[cfg(target_arch = "riscv64")]
        crate::arch::rv64::registers::satp::write(args.0.as_usize(), args.1, args.2);
    }

    #[inline(always)]
    pub const fn frame(&self) -> Address<Frame> {
        self.0
    }
}

pub fn supports_5_level_paging() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        crate::arch::x64::cpuid::EXT_FEATURE_INFO
            .as_ref()
            .map(|ext_feature_info| ext_feature_info.has_la57())
            .unwrap_or(false)
    }

    #[cfg(target_arch = "riscv64")]
    {
        todo!()
    }
}

pub fn is_5_level_paged() -> bool {
    #[cfg(target_arch = "x86_64")]
    {
        supports_5_level_paging()
            && crate::arch::x64::registers::control::CR4::read()
                .contains(crate::arch::x64::registers::control::CR4Flags::LA57)
    }
}

pub mod allocator {
    use core::alloc::Allocator;

    pub struct AlignedAllocator<const ALIGN: usize, A: Allocator>(pub A);

    unsafe impl<const ALIGN: usize, A: Allocator> Allocator for AlignedAllocator<ALIGN, A> {
        fn allocate(&self, layout: core::alloc::Layout) -> Result<core::ptr::NonNull<[u8]>, core::alloc::AllocError> {
            match layout.align_to(ALIGN) {
                Ok(layout) => self.0.allocate(layout),
                Err(_) => Err(core::alloc::AllocError),
            }
        }

        unsafe fn deallocate(&self, ptr: core::ptr::NonNull<u8>, layout: core::alloc::Layout) {
            match layout.align_to(ALIGN) {
                Ok(layout) => self.0.deallocate(ptr, layout),
                Err(_) => unimplemented!(),
            }
        }
    }

    pub static KERNEL_ALLOCATOR: spin::Lazy<super::slab::SlabAllocator> = spin::Lazy::new(|| {
        let memory_map =
            crate::boot::get_memory_map().expect("kernel allocator requires boot loader memory map for initialization");

        unsafe { crate::memory::slab::SlabAllocator::from_memory_map(memory_map, crate::memory::get_hhdm_address()) }
            .unwrap_or_else(|| todo!("fall back to a simpler allocator"))
    });
}
