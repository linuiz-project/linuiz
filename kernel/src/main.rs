#![no_std]
#![no_main]
#![feature(abi_efiapi, abi_x86_interrupt, once_cell, const_mut_refs, raw_ref_op)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libstd;

mod block_malloc;
mod drivers;
mod logging;
mod timer;

use libstd::{
    acpi::SystemConfigTableEntry,
    cell::SyncOnceCell,
    memory::{falloc, malloc::MemoryAllocator, UEFIMemoryDescriptor},
    BootInfo, LinkerSymbol,
};

extern "C" {
    static __ap_trampoline_start: LinkerSymbol;
    static __ap_trampoline_end: LinkerSymbol;
    static __kernel_pml4: LinkerSymbol;
    #[link_name = "__gdt.pointer"]
    static __gdt_pointer: LinkerSymbol;
    #[link_name = "__gdt.code"]
    static __gdt_code: LinkerSymbol;
    #[link_name = "__gdt.data"]
    static __gdt_data: LinkerSymbol;

    static __bsp_stack_start: LinkerSymbol;
    static __bsp_stack_end: LinkerSymbol;

    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;

    static __rodata_start: LinkerSymbol;
    static __rodata_end: LinkerSymbol;

    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;

    static __bss_start: LinkerSymbol;
    static __bss_end: LinkerSymbol;
}

#[export_name = "__ap_stack_pointers"]
static mut AP_STACK_POINTERS: [*const (); 256] = [core::ptr::null(); 256];

fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Debug
}

static mut CON_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);
static TRACE_ENABLED_PATHS: [&str; 1] = ["libstd::structures::apic::icr"];
static BOOT_INFO: SyncOnceCell<BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>> =
    SyncOnceCell::new();
static KERNEL_MALLOCATOR: SyncOnceCell<block_malloc::BlockAllocator> = SyncOnceCell::new();

/// Clears the kernel stack by resetting `RSP`.
///
/// Safety: This method does *extreme* damage to the stack. It should only ever be used when
///         ABSOLUTELY NO dangling references to the old stack will exist (i.e. calling a
///         no-argument function directly after).
#[inline(always)]
unsafe fn clear_stack() {
    libstd::registers::stack::RSP::write(__bsp_stack_end.as_ptr());
}

#[no_mangle]
#[export_name = "_entry"]
unsafe extern "efiapi" fn _kernel_pre_init(
    boot_info: BootInfo<UEFIMemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    if let Err(_) = BOOT_INFO.set(boot_info) {
        libstd::instructions::interrupts::breakpoint();
    }

    clear_stack();
    kernel_init()
}

unsafe fn kernel_init() -> ! {
    CON_OUT.init(drivers::io::SerialSpeed::S115200);

    match drivers::io::set_stdout(&mut CON_OUT, get_log_level(), &TRACE_ENABLED_PATHS) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => libstd::instructions::interrupts::breakpoint(),
    }

    {
        libstd::instructions::segmentation::lgdt(
            __gdt_pointer
                .as_ptr::<libstd::structures::gdt::DescriptorTablePointer>()
                .as_ref()
                .unwrap(),
        );
        libstd::instructions::init_segment_registers(__gdt_data.as_usize() as u16);
        use x86_64::instructions::segmentation::Segment;
        x86_64::instructions::segmentation::CS::set_reg(core::mem::transmute(
            __gdt_code.as_usize() as u16,
        ));
    }

    let boot_info = BOOT_INFO
        .get()
        .expect("Boot info hasn't been initialized in kernel memory");

    info!("Validating BootInfo struct.");
    boot_info.validate_magic();

    debug!(
        "Detected CPU features: {:?}",
        libstd::instructions::cpu_features()
    );

    debug!("Initializing kernel frame allocator.");
    falloc::load_new(boot_info.memory_map());
    init_system_config_table(boot_info.config_table());

    clear_stack();
    kernel_mem_init()
}

#[inline(never)]
unsafe fn kernel_mem_init() -> ! {
    info!("Initializing kernel default allocator.");

    let malloc = block_malloc::BlockAllocator::new();
    debug!("Flagging `text` and `rodata` kernel sections as read-only.");
    use libstd::memory::Page;
    let text_page_range = Page::from_index(__text_start.as_usize() / 0x1000)
        ..=Page::from_index(__text_end.as_usize() / 0x1000);
    let rodata_page_range = Page::from_index(__rodata_start.as_usize() / 0x1000)
        ..=Page::from_index(__rodata_end.as_usize() / 0x1000);
    for page in text_page_range.chain(rodata_page_range) {
        malloc.set_page_attribs(
            &page,
            libstd::memory::paging::PageAttributes::WRITABLE,
            libstd::memory::paging::AttributeModify::Remove,
        );
    }

    debug!("Setting libstd's default memory allocator to new kernel allocator.");
    KERNEL_MALLOCATOR.set(malloc).map_err(|_| panic!()).ok();
    libstd::memory::malloc::set(KERNEL_MALLOCATOR.get().unwrap());
    // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
    debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
    __kernel_pml4
        .as_mut_ptr::<u32>()
        .write(libstd::registers::CR3::read().0.as_usize() as u32);

    info!("Finalizing kernel memory allocator initialization.");

    clear_stack();
    _startup()
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    unsafe { libstd::registers::debug::DR0::write(2) };
    unsafe { libstd::structures::idt::load() };
    unsafe { libstd::registers::debug::DR0::write(3) };
    libstd::lpu::init();
    unsafe { libstd::registers::debug::DR0::write(4) };
    init_apic();
    unsafe { libstd::registers::debug::DR0::write(5) };

    // If this is the BSP, wake other cores.
    if libstd::lpu::is_bsp() {
        use libstd::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        let apic = libstd::lpu::try_get().unwrap().apic();
        let icr = apic.interrupt_command_register();
        let ap_trampoline_page_index = unsafe { __ap_trampoline_start.as_page().index() } as u8;

        if let Ok(madt) = XSDT.find_sub_table::<MADT>() {
            info!("Beginning wake-up sequence for enabled processors.");
            for interrupt_device in madt.iter() {
                if let InterruptDevice::LocalAPIC(apic_other) = interrupt_device {
                    use libstd::acpi::rdsp::xsdt::madt::LocalAPICFlags;

                    // Ensure the CPU core can actually be enabled.
                    if apic_other.flags().intersects(
                        LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                    ) && apic.id() != apic_other.id()
                    {
                        unsafe {
                            const AP_STACK_SIZE: usize = 0x2000;

                            let (stack_bottom, len) = libstd::memory::malloc::try_get()
                                .unwrap()
                                .alloc(AP_STACK_SIZE, core::num::NonZeroUsize::new(0x1000))
                                .expect("Failed to allocate stack for LPU")
                                .into_parts();

                            AP_STACK_POINTERS[apic_other.id() as usize] =
                                stack_bottom.add(len) as *mut _;
                        };

                        icr.send_init(apic_other.id());
                        icr.wait_pending();

                        icr.send_sipi(ap_trampoline_page_index, apic_other.id());
                        icr.wait_pending();
                        icr.send_sipi(ap_trampoline_page_index, apic_other.id());
                        icr.wait_pending();
                    }
                }
            }
        }
    }

    // if libstd::lpu::is_bsp() {
    //     use libstd::{
    //         acpi::rdsp::xsdt::{mcfg::MCFG, XSDT},
    //         io::pci,
    //     };

    //     if let Ok(mcfg) = XSDT.find_sub_table::<MCFG>() {
    //         let bridges: alloc::vec::Vec<pci::PCIeHostBridge> = mcfg
    //             .iter()
    //             .filter_map(|entry| pci::configure_host_bridge(entry).ok())
    //             .collect();

    //         for device_variant in bridges
    //             .iter()
    //             .flat_map(|bridge| bridge.iter())
    //             .flat_map(|bus| bus.iter())
    //         {
    //             if let pci::DeviceVariant::Standard(device) = device_variant {
    //                 if device.class() == pci::DeviceClass::MassStorageController
    //                     && device.subclass() == 0x08
    //                 {
    //                     // // NVMe device

    //                     // use crate::drivers::nvme::*;

    //                     // let mut nvme = Controller::from_device(&device);

    //                     // let admin_sq = libstd::slice!(u8, 0x1000);
    //                     // let admin_cq = libstd::slice!(u8, 0x1000);

    //                     // let cc = nvme.controller_configuration();
    //                     // cc.set_iosqes(4);
    //                     // cc.set_iocqes(4);

    //                     // if unsafe { !nvme.safe_set_enable(true) } {
    //                     //     error!("NVMe controleler failed to safely enable.");
    //                     //     break;
    //                     // }
    //                 }
    //             }
    //         }
    //     }
    // }

    libstd::instructions::hlt_indefinite()
}

fn init_system_config_table(config_table: &[SystemConfigTableEntry]) {
    info!("Initializing system configuration table.");
    let config_table_ptr = config_table.as_ptr();
    let config_table_entry_len = config_table.len();

    let frame_index = (config_table_ptr as usize) / 0x1000;
    let frame_count =
        (config_table_entry_len * core::mem::size_of::<SystemConfigTableEntry>()) / 0x1000;

    unsafe {
        // Assign system configuration table prior to reserving frames to ensure one doesn't already exist.
        libstd::acpi::init_system_config_table(config_table_ptr, config_table_entry_len);

        let frame_range = frame_index..(frame_index + frame_count);
        debug!("System configuration table: {:?}", frame_range);
        let falloc = falloc::get();
        for frame_index in frame_index..(frame_index + frame_count) {
            falloc.borrow(frame_index).unwrap();
        }
    }
}

fn init_apic() {
    use libstd::structures::idt;

    let apic = &libstd::lpu::try_get().unwrap().apic();

    apic.auto_configure_timer_frequency();

    idt::set_interrupt_handler(32, timer::apic_tick_handler);
    apic.timer().set_vector(32);
    idt::set_interrupt_handler(58, apic_error_handler);
    apic.error().set_vector(58);

    apic.timer()
        .set_mode(libstd::structures::apic::TimerMode::Periodic);
    apic.timer().set_masked(false);
    apic.sw_enable();

    info!("Core-local APIC configured and enabled.");
}

extern "x86-interrupt" fn apic_error_handler(_: libstd::structures::idt::InterruptStackFrame) {
    let apic = &libstd::lpu::try_get().unwrap().apic();

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}
