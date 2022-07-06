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
    core_c_str
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod clock;
mod drivers;
mod local_state;
mod logging;
mod scheduling;
mod slob;
mod syscall;
mod tables;

use alloc::vec::Vec;
use libkernel::memory::{Page, PageManager};
use libkernel::{acpi::SystemConfigTableEntry, LinkerSymbol};
use libkernel::{Address, Virtual};

extern "C" {
    static __code_start: LinkerSymbol;
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
#[used]
static LIMINE_INF: limine::LimineBootInfoRequest = limine::LimineBootInfoRequest::new(LIMINE_REV);
#[used]
static LIMINE_FB: limine::LimineFramebufferRequest =
    limine::LimineFramebufferRequest::new(LIMINE_REV);
#[used]
static LIMINE_SMP: limine::LimineSmpRequest = limine::LimineSmpRequest::new(LIMINE_REV);
#[used]
static LIMINE_SYS_TBL: limine::LimineEfiSystemTableRequest =
    limine::LimineEfiSystemTableRequest::new(LIMINE_REV);
#[used]
static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(LIMINE_REV);

static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);

lazy_static::lazy_static! {
    pub static ref KMALLOC: slob::SLOB<'static> = slob::SLOB::new();
}

use libkernel::io::pci;
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
//             libkernel::io::pci::get_pcie_devices(memory::get_frame_manager(), &*crate::memory::PAGE_MANAGER, &*crate::memory::KMALLOC).collect(),
//             &core::marker::PhantomData
//         );
// }

fn load_registers() {
    unsafe {
        use libkernel::cpu::{has_feature, Feature};

        // Set CR0 flags.
        use libkernel::registers::control::{CR0Flags, CR0};
        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
        // Set CR4 flags.
        use libkernel::registers::control::{CR4Flags, CR4};
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
            libkernel::registers::msr::IA32_EFER::set_nxe(true);
        } else {
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).")
        }
    }
}

fn load_tables() {
    unsafe {
        use tables::{gdt, idt};

        // Always initialize GDT prior to configuring IDT.
        gdt::init();

        if libkernel::cpu::is_bsp() {
            // Due to the fashion in which the `x86_64` crate initializes the IDT entries,
            // it must be ensured that the handlers are set only *after* the GDT has been
            // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
            // is incorrect, and this causes very confusing GPFs.
            idt::init();

            fn apit_empty(
                _: &mut x86_64::structures::idt::InterruptStackFrame,
                _: *mut scheduling::ThreadRegisters,
            ) {
                libkernel::structures::apic::APIC::end_of_interrupt();
            }

            idt::set_handler_fn(local_state::InterruptVector::LINT0 as u8, apit_empty);
            idt::set_handler_fn(local_state::InterruptVector::LINT1 as u8, apit_empty);
        }

        crate::tables::idt::load();
    }
}

#[no_mangle]
unsafe extern "sysv64" fn _entry() -> ! {
    // Load the pre-reserved BSP stack (lies within the kernel's .bss, so is loaded by program segments).
    libkernel::registers::stack::RSP::write(__bsp_top.as_mut_ptr());

    // Drop into initialization function to completely invalidate any previous stack state.
    init()
}

unsafe fn load_system_table() {
    if let Some(system_table_ptr) = LIMINE_SYS_TBL
        .get_response()
        .get()
        .and_then(|resp| resp.address.as_ptr())
    {
        debug!("Loading EFI system configuration table...");
        let system_table_ptr = system_table_ptr as *const SystemConfigTableEntry;

        let mut system_table_len = 0;
        while let Some(system_config_entry) = system_table_ptr.add(system_table_len).as_ref() {
            // REMARK: There may be a better way to check for the end of the system table? I'm not sure this is always valid.
            if system_config_entry.addr().is_null() {
                break;
            } else {
                system_table_len += 1;
            }
        }

        // TODO Possibly move ACPI structure instances out of libkernel?
        // Set system configuration table, so ACPI can be used.
        libkernel::acpi::set_system_config_table(core::slice::from_raw_parts(
            system_table_ptr,
            system_table_len,
        ));
    } else {
        warn!("No EFI system configuration table found.");
    }
}

unsafe fn init_memory() {
    if let Some(memory_map) = LIMINE_MMAP
        .get_response()
        .get()
        .and_then(|resp| resp.mmap())
    {
        use libkernel::{
            align_down_div, align_up_div,
            memory::{AttributeModify, Page, PageAttributes},
        };

        // We always ensure the frame manager is initialized first—all other memory structures
        // rely on its validity.
        libkernel::memory::init_frame_manager(memory_map);

        debug!("Global mapping kernel ELF pages.");

        let boot_pmgr =
            libkernel::memory::PageManager::from_current(&libkernel::memory::Page::null());
        let global_pmgr = libkernel::memory::global_pmgr();
        // Create ranges of pages, using pre-defined linker symbols to demarkate what memory zones
        // need to be mapped in the kernel's page manager.
        //
        // The bootloader will (generally) do KASLR, so it cannot be relied upon that static executable
        // compilation will run successfully; thus, the kernel is compiled PIE, and the linker file
        // provides a method by which the kernel can continue to manage its own sections.
        let kernel_code = Page::range(
            align_down_div(__code_start.as_usize(), 0x1000),
            align_up_div(__code_end.as_usize(), 0x1000),
        );
        let kernel_ro = Page::range(
            align_down_div(__ro_start.as_usize(), 0x1000),
            align_up_div(__ro_end.as_usize(), 0x1000),
        );
        let kernel_relro = Page::range(
            align_down_div(__relro_start.as_usize(), 0x1000),
            align_up_div(__relro_end.as_usize(), 0x1000),
        );
        let kernel_rw = Page::range(
            align_down_div(__rw_start.as_usize(), 0x1000),
            align_up_div(__rw_end.as_usize(), 0x1000),
        );

        for page in kernel_code {
            global_pmgr
                .map(
                    &page,
                    boot_pmgr
                        .get_mapped_to(&page)
                        .expect("kernel code page not mapped in bootloader page tables"),
                    None,
                    PageAttributes::CODE | PageAttributes::GLOBAL,
                )
                .unwrap();
        }

        for page in kernel_ro {
            global_pmgr
                .map(
                    &page,
                    boot_pmgr
                        .get_mapped_to(&page)
                        .expect("kernel readonly page not mapped in bootloader page tables"),
                    None,
                    PageAttributes::RODATA | PageAttributes::GLOBAL,
                )
                .unwrap();
        }

        for page in kernel_rw.chain(kernel_relro) {
            global_pmgr
                .map(
                    &page,
                    boot_pmgr
                        .get_mapped_to(&page)
                        .expect("kernel rw/relro page not mapped in bootloader page tables"),
                    None,
                    PageAttributes::DATA | PageAttributes::GLOBAL,
                )
                .unwrap();
        }

        libkernel::memory::finalize_paging();

        let frame_manager = libkernel::memory::global_fmgr();
        // Make sure global PML4 has all L4 entries mapped to a frame (so core-local
        // PML4 copies share parity of address space).
        let pml4 = (libkernel::registers::control::CR3::read().0.as_u64() as *mut u8)
            .add(global_pmgr.mapped_page().index() * 0x1000)
            .cast::<libkernel::memory::PageTable<libkernel::memory::Level4>>()
            .as_mut()
            .unwrap();
        for entry in pml4.iter_mut().take(256) {
            entry.set(
                frame_manager.lock_next().unwrap(),
                PageAttributes::PRESENT | PageAttributes::WRITABLE | PageAttributes::USERSPACE,
            );
        }
        // Reclaim bootloader memory.
        for (index, (ty, _, _)) in frame_manager.iter().enumerate() {
            if ty == libkernel::memory::FrameType::BootReclaim {
                frame_manager
                    .try_modify_type(index, libkernel::memory::FrameType::Usable)
                    .unwrap();
            }
        }

        libkernel::memory::global_alloc::set(&*KMALLOC);

        info!("Swapped memory control to kernel.");
    } else {
        panic!("No memory map has been provided by bootloader.");
    }
}

unsafe fn init() -> ! {
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);
    match drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => libkernel::instructions::interrupts::breakpoint(),
    }

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
    info!("CPU Vendor          {}", libkernel::cpu::VENDOR);
    info!("CPU Features        {:?}", libkernel::cpu::FeatureFmt);

    load_system_table();
    init_memory();

    _startup()
}

#[no_mangle]
#[inline(never)]
unsafe extern "C" fn _startup() -> ! {
    use libkernel::cpu::is_bsp;

    // BSP should have already loaded these.

    load_registers();
    load_tables();

    local_state::create();

    /* LOAD TSS */
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

    /* INIT APIC */
    {
        // TODO
        // local_state::init_local_apic();
        // local_state::reload_timer(core::num::NonZeroU32::new(1));
    }
    if is_bsp() {
        // TODO wake_aps();
    }

    loop {}

    /* ENABLE SYSCALL */
    {
        use crate::tables::gdt;
        use libkernel::registers::msr;

        // Enable `syscall`/`sysret`.
        msr::IA32_EFER::set_sce(true);
        // Configure system call environment registers.
        msr::IA32_STAR::set_selectors(
            *gdt::KCODE_SELECTOR.get().unwrap(),
            *gdt::KDATA_SELECTOR.get().unwrap(),
        );
        msr::IA32_LSTAR::set_syscall(syscall::syscall_enter);
        msr::IA32_SFMASK::set_rflags_mask(libkernel::registers::RFlags::all());
    }

    libkernel::registers::stack::RSP::write(libkernel::memory::alloc_stack(1, true));
    libkernel::cpu::ring3_enter(test_user_function, libkernel::registers::RFlags::empty());

    debug!("Failed to enter ring 3.");

    libkernel::instructions::hlt_indefinite()
}

// unsafe fn wake_aps() {
//     use libkernel::acpi::rdsp::xsdt::{
//         madt::{InterruptDevice, MADT},
//         XSDT,
//     };

//     let lapic_id = libkernel::cpu::get_id() as u8 /* possibly don't cast to u8? */;
//     let icr = libkernel::structures::apic::APIC::interrupt_command_register();
//     let ap_text_page_index = (__ap_text_start.as_usize() / 0x1000) as u8;

//     if let Some(madt) = XSDT.find_sub_table::<MADT>() {
//         info!("Beginning wake-up sequence for enabled processors.");
//         for interrupt_device in madt.iter() {
//             // Filter out non-lapic devices.
//             if let InterruptDevice::LocalAPIC(ap_lapic) = interrupt_device {
//                 use libkernel::acpi::rdsp::xsdt::madt::LocalAPICFlags;
//                 // Filter out invalid lapic devices.
//                 if lapic_id != ap_lapic.id()
//                     && ap_lapic.flags().intersects(
//                         LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
//                     )
//                 {
//                     debug!("Waking core ID {}.", ap_lapic.id());

//                     AP_STACK_POINTERS[ap_lapic.id() as usize] =
//                         libkernel::memory::alloc_stack(2, false);

//                     info!("{:?}", AP_STACK_POINTERS[ap_lapic.id() as usize]);

//                     // Reset target processor.
//                     trace!("Sending INIT interrupt to: {}", ap_lapic.id());
//                     icr.send_init(ap_lapic.id());
//                     icr.wait_pending();
//                     // REMARK: IA32 spec indicates that doing this twice, as so, ensures the interrupt is received.
//                     trace!("Sending SIPI x1 interrupt to: {}", ap_lapic.id());
//                     icr.send_sipi(ap_text_page_index, ap_lapic.id());
//                     icr.wait_pending();
//                     trace!("Sending SIPI x2 interrupt to: {}", ap_lapic.id());
//                     icr.send_sipi(ap_text_page_index, ap_lapic.id());
//                     icr.wait_pending();
//                 }
//             }
//         }
//     }
// }

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    libkernel::instructions::hlt_indefinite()
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

    libkernel::instructions::interrupts::breakpoint();

    loop {}
}
