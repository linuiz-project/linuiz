#![no_std]
#![no_main]
#![feature(
    abi_efiapi,
    abi_x86_interrupt,
    once_cell,
    const_mut_refs,
    raw_ref_op,
    const_option_ext,
    naked_functions,
    asm_sym,
    asm_const,
    const_ptr_offset_from,
    const_refs_to_cell,
    core_c_str,
    exclusive_range_pattern,
    raw_vec_internals
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate liblz;

mod clock;
mod drivers;
mod local_state;
mod logging;
mod scheduling;
mod slob;
mod tables;

use alloc::vec::Vec;
use core::sync::atomic::AtomicBool;
use liblz::LinkerSymbol;

extern "C" {
    static __code_start: LinkerSymbol;
    static __startup_start: LinkerSymbol;
    static __code_end: LinkerSymbol;

    static __ro_start: LinkerSymbol;
    static __ro_end: LinkerSymbol;

    static __relro_start: LinkerSymbol;
    static __relro_end: LinkerSymbol;

    static __rw_start: LinkerSymbol;
    static __bsp_top: LinkerSymbol;
    static __rw_end: LinkerSymbol;
}

const LIMINE_REV: u64 = 0;
static LIMINE_INF: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
static LIMINE_FB: limine::LimineFramebufferRequest =
    limine::LimineFramebufferRequest::new(LIMINE_REV);
static LIMINE_SMP: limine::LimineSmpRequest = {
    let mut req = limine::LimineSmpRequest::new(LIMINE_REV);
    req.flags = 0b1; // Enable x2APIC.
    req
};
static LIMINE_RSDP: limine::LimineRsdpRequest = limine::LimineRsdpRequest::new(LIMINE_REV);
static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(LIMINE_REV);
static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(LIMINE_REV);

static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);

static SMP_MEMORY_READY: AtomicBool = AtomicBool::new(false);

lazy_static::lazy_static! {
    /// We must take care not to call any allocating functions, or reference KMALLOC itself,
    /// prior to initializing memory (frame/page manager). The SLOB *immtediately* configures
    /// its own allocation table, utilizing both of the aforementioned managers.
    pub static ref KMALLOC: slob::SLOB<'static> = unsafe { slob::SLOB::new() };
}

use liblz::io::pci;
pub struct Devices<'a>(Vec<pci::DeviceVariant>, &'a core::marker::PhantomData<()>);
unsafe impl Send for Devices<'_> {}
unsafe impl Sync for Devices<'_> {}

impl Devices<'_> {
    pub fn iter(&self) -> core::slice::Iter<pci::DeviceVariant> {
        self.0.iter()
    }
}

// lazy_static::lazy_static! {
//     pub static ref PCIE_DEVICES: Devices<'static> =
//         Devices(
//             liblz::io::pci::get_pcie_devices(memory::get_frame_manager(), &*crate::memory::PAGE_MANAGER, &*crate::memory::KMALLOC).collect(),
//             &core::marker::PhantomData
//         );
// }

#[no_mangle]
unsafe extern "sysv64" fn _entry() -> ! {
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);
    match drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => liblz::instructions::interrupts::breakpoint(),
    }

    /* log boot info */
    {
        let boot_info = LIMINE_INF
            .get_response()
            .get()
            .expect("bootloader provided no info");
        info!(
            "Bootloader Info     {:?} (rev {:?}) {:?}",
            core::ffi::CStr::from_ptr(boot_info.name.as_ptr().unwrap() as *const _),
            boot_info.revision,
            core::ffi::CStr::from_ptr(boot_info.version.as_ptr().unwrap() as *const _)
        );
        info!("CPU Vendor          {}", liblz::cpu::VENDOR);
        info!(
            "CPU x2APIC          {}",
            if liblz::registers::msr::IA32_APIC_BASE::is_x2_mode() {
                "Yes"
            } else {
                "No"
            }
        );
        info!("CPU Features        {:?}", liblz::cpu::FeatureFmt);
    }

    /* prepare APs for startup */
    {
        let smp_response = LIMINE_SMP
            .get_response()
            .as_mut_ptr()
            .expect("received no SMP response from bootloader")
            .as_mut()
            .unwrap();

        if (smp_response.flags & 0b1) == 0 {
            panic!("x2APIC mode has failed to enable. Kernel does not support xAPIC mode.");
        }

        if let Some(cpus) = smp_response.cpus() {
            debug!("Detected {} processors.", cpus.len());

            for cpu_info in cpus {
                debug!(
                    "Starting processor: PID{}/LID{}",
                    cpu_info.processor_id, cpu_info.lapic_id
                );
                cpu_info.goto_address = _cpu_entry as u64;
            }
        }
    }

    /* load RSDP pointer */
    {
        // TODO Possibly move ACPI structure instances out of liblz?
        // Set RSDP pointer, so ACPI can be used.
        liblz::acpi::set_rsdp_ptr(
            LIMINE_RSDP
                .get_response()
                .get()
                .expect("bootloader provided to RSDP pointer (no ACPI support)")
                .address
                .as_ptr()
                .unwrap() as *const _,
        );
    }

    /* init memory */
    {
        use liblz::memory::Page;

        let memory_map = LIMINE_MMAP
            .get_response()
            .get()
            .and_then(|resp| resp.mmap())
            .expect("no memory map has been provided by bootloader");

        trace!("Initializing memory managers.");
        // Frame manager is always initialized first, so memory structures may allocate frames.
        liblz::memory::init_frame_manager(memory_map);
        let hhdm_offset = LIMINE_HHDM
            .get_response()
            .get()
            .expect("bootloader did not provide a higher half direct mapping")
            .offset as usize;
        let hhdm_page = Page::from_index(hhdm_offset / 0x1000);
        liblz::memory::init_page_manager(&hhdm_page, false);
        // The frame manager's allocation table is allocated with identity mapping assumed,
        // so before we unmap the lower half virtual memory mapping, we must ensure the
        // frame manager uses the HHDM base.
        liblz::memory::global_fmgr().slide_table_base(hhdm_offset);

        trace!("Unmapping lower half identity mappings.");
        let global_pmgr = liblz::memory::global_pmgr();
        for entry in memory_map.iter() {
            for page in (entry.base..(entry.base + entry.len))
                .step_by(0x1000)
                .map(|base| Page::from_index((base / 0x1000) as usize))
            {
                global_pmgr
                    .unmap(&page, liblz::memory::FrameOwnership::None)
                    .ok();
            }
        }

        // The global kernel allocator must be set AFTER the upper half
        // identity mappings are purged, so that the allocation table
        // isn't unmapped.
        liblz::memory::global_alloc::set(&*KMALLOC);

        // TODO cleanup bootloader reclaimable memory
        // trace!("Reclaiming bootloader memory.");
        // for (index, (ty, _, _)) in global_fmgr.iter().enumerate() {
        //     if ty == liblz::memory::FrameType::BootReclaim {
        //         global_fmgr
        //             .try_modify_type(index, liblz::memory::FrameType::Usable)
        //             .unwrap();
        //     }
        // }
    }

    debug!("Finished initial kernel setup.");
    SMP_MEMORY_READY.store(true, core::sync::atomic::Ordering::Relaxed);
    _cpu_entry()
}

#[inline(never)]
unsafe extern "C" fn _cpu_entry() -> ! {
    while !SMP_MEMORY_READY.load(core::sync::atomic::Ordering::Relaxed) {}

    /* load registers */
    {
        use liblz::cpu::{has_feature, Feature};

        // Set CR0 flags.
        use liblz::registers::control::{CR0Flags, CR0};
        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
        // Set CR4 flags.
        use liblz::registers::control::{CR4Flags, CR4};
        CR4::write(
            CR4Flags::DE
                | CR4Flags::PAE
                | CR4Flags::MCE
                | CR4Flags::PGE
                | CR4Flags::OSFXSR
                | CR4Flags::OSXMMEXCPT
                | CR4Flags::UMIP
                | if has_feature(Feature::FSGSBASE) {
                    CR4Flags::FSGSBASE
                } else {
                    CR4Flags::empty()
                },
        );

        // Enable use of the `NO_EXECUTE` page attribute, if supported.
        if has_feature(Feature::NXE) {
            liblz::registers::msr::IA32_EFER::set_nxe(true);
        } else {
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
        }

        liblz::instructions::interrupts::enable();
    }

    /* load tables */
    {
        use tables::{gdt, idt};

        // Always initialize GDT prior to configuring IDT.
        gdt::init();

        if liblz::cpu::is_bsp() {
            // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
            // it must be ensured that the handlers are set only *after* the GDT has been
            // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
            // is incorrect, and this causes very confusing GPFs.
            idt::init();

            fn apit_empty(
                _: &mut x86_64::structures::idt::InterruptStackFrame,
                _: *mut scheduling::ThreadRegisters,
            ) {
                liblz::structures::apic::end_of_interrupt();
            }

            idt::set_handler_fn(liblz::structures::apic::LINT0_VECTOR, apit_empty);
            idt::set_handler_fn(liblz::structures::apic::LINT1_VECTOR, apit_empty);
        }

        crate::tables::idt::load();
    }

    if liblz::cpu::is_bsp() {
        crate::clock::global::configure_and_enable();
    }

    local_state::create();
    liblz::structures::apic::get_timer().set_masked(false);
    local_state::reload_timer(core::num::NonZeroU32::new(1).unwrap());

    /* load tss */
    {
        use x86_64::{
            instructions::tables,
            structures::{
                gdt::{Descriptor, GlobalDescriptorTable},
                tss::TaskStateSegment,
            },
        };

        let tss_ptr = {
            use alloc::boxed::Box;
            Box::leak(Box::new(TaskStateSegment::new())) as *mut TaskStateSegment
        };

        tss_ptr.as_mut().unwrap().privilege_stack_table[0] =
            x86_64::VirtAddr::from_ptr(crate::local_state::privilege_stack().as_ptr());

        let tss_descriptor = {
            use bit_field::BitField;

            let tss_ptr_u64 = tss_ptr as u64;

            let mut low = x86_64::structures::gdt::DescriptorFlags::PRESENT.bits();
            // base
            low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
            low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
            // limit (the `-1` is needed since the bound is inclusive, not exclusive)
            low.set_bits(0..16, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);
            // type (0b1001 = available 64-bit tss)
            low.set_bits(40..44, 0b1001);

            // high 32 bits of base
            let mut high = 0;
            high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

            Descriptor::SystemSegment(low, high)
        };

        // Store current GDT pointer to restore later.
        let cur_gdt = tables::sgdt();
        // Create temporary kernel GDT to avoid a GPF on switching to it.
        let mut temp_gdt = GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);

        // Load temp GDT ...
        temp_gdt.load_unsafe();
        // ... load TSS from temporary GDT ...
        tables::load_tss(tss_selector);
        // ... and restore cached GDT.
        tables::lgdt(&cur_gdt);
    }

    cpu_setup()
}

fn one() -> ! {
    loop {
        info!("TEST1");

        clock::global::busy_wait_msec(500);
    }
}

fn two() -> ! {
    loop {
        info!("TEST2");
        clock::global::busy_wait_msec(500);
    }
}

#[inline(never)]
unsafe fn cpu_setup() -> ! {
    if liblz::cpu::is_bsp() {
        use liblz::registers::RFlags;
        use scheduling::{Task, TaskPriority, SCHEDULER};

        SCHEDULER.push_task(Task::new(
            TaskPriority::new(5).unwrap(),
            one,
            None,
            RFlags::INTERRUPT_FLAG,
            *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
            *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
            liblz::registers::control::CR3::read(),
        ));
        // SCHEDULER.push_task(Task::new(
        //     TaskPriority::new(7).unwrap(),
        //     two,
        //     None,
        //     RFlags::INTERRUPT_FLAG,
        //     *crate::tables::gdt::KCODE_SELECTOR.get().unwrap(),
        //     *crate::tables::gdt::KDATA_SELECTOR.get().unwrap(),
        //     liblz::registers::control::CR3::read(),
        // ));
        SCHEDULER.enable();
    }

    liblz::instructions::hlt_indefinite();

    /* ENABLE SYSCALL */
    // {
    //     use crate::tables::gdt;
    //     use liblz::registers::msr;

    //     // Enable `syscall`/`sysret`.
    //     msr::IA32_EFER::set_sce(true);
    //     // Configure system call environment registers.
    //     msr::IA32_STAR::set_selectors(
    //         *gdt::KCODE_SELECTOR.get().unwrap(),
    //         *gdt::KDATA_SELECTOR.get().unwrap(),
    //     );
    //     msr::IA32_LSTAR::set_syscall(syscall::syscall_enter);
    //     msr::IA32_SFMASK::set_rflags_mask(liblz::registers::RFlags::all());
    // }

    // liblz::registers::stack::RSP::write(liblz::memory::alloc_stack(1, true));
    // liblz::cpu::ring3_enter(test_user_function, liblz::registers::RFlags::empty());

    debug!("Failed to enter ring 3.");

    liblz::instructions::hlt_indefinite()
}

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    liblz::instructions::hlt_indefinite()
}

#[link_section = ".user_code"]
fn test_user_function() {
    // unsafe {
    //     core::arch::asm!(
    //         "mov r10, $0",
    //         "mov r8,   0x1F1F1FA1",
    //         "mov r9,   0x1F1F1FA2",
    //         "mov r13,   0x1F1F1FA3",
    //         "mov r14,   0x1F1F1FA4",
    //         "mov r15,   0x1F1F1FA5",
    //         "syscall",
    //         out("rcx") _,
    //         out("rdx") _,
    //         out("r10") _,
    //         out("r11") _,
    //         out("r12") _,
    //     )
    // };

    liblz::instructions::interrupts::breakpoint();

    loop {}
}
