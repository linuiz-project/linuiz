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
mod lpu;

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

    debug!("CPU features: {:?}", libstd::instructions::cpuid::FEATURES);

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
    unsafe { libstd::structures::idt::load() };
    crate::lpu::init(alloc::boxed::Box::new(drivers::clock::MSRClock::new(1000)));
    init_apic();

    // If this is the BSP, wake other cores.
    if crate::lpu::is_bsp() {
        use libstd::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        let apic = crate::lpu::try_get().unwrap().apic();
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

            if lpu::LPU_COUNT.load(core::sync::atomic::Ordering::Relaxed) == 1 {
                info!("Single-core CPU detected. No multiprocessing will occur.");
                // TODO somehow handle single-core.
            }
        }
    }

    if crate::lpu::is_bsp() && false {
        use libstd::{
            acpi::rdsp::xsdt::{mcfg::MCFG, XSDT},
            io::pci,
        };

        if let Ok(mcfg) = XSDT.find_sub_table::<MCFG>() {
            let bridges: alloc::vec::Vec<pci::PCIeHostBridge> = mcfg
                .iter()
                .filter_map(|entry| pci::configure_host_bridge(entry).ok())
                .collect();

            for device_variant in bridges
                .iter()
                .flat_map(|bridge| bridge.iter())
                .flat_map(|bus| bus.iter())
            {
                if let pci::DeviceVariant::Standard(device) = device_variant {
                    if device.class() == pci::DeviceClass::MassStorageController
                        // Serial ATA Controller
                        && device.subclass() == 0x08
                    {
                        use crate::drivers::nvme::Controller;
                        let nvme = Controller::from_device(device, 4, 4);

                        info!("{:#?}", nvme);
                    }
                }
            }
        }
    }

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
    use libstd::{
        registers::MSR,
        structures::{apic::*, idt, pic8259},
    };

    extern "x86-interrupt" fn pit_tick_handler(_: idt::InterruptStackFrame) {
        unsafe { MSR::IA32_GS_BASE.write(MSR::IA32_GS_BASE.read() + 1) };
        pic8259::end_of_interrupt(pic8259::InterruptOffset::Timer);
    }

    libstd::instructions::interrupts::disable();
    let lpu = crate::lpu::try_get().unwrap();
    let apic = lpu.apic();

    trace!("Resetting and enabling local APIC (it may have already been enabled).");
    unsafe { apic.reset() };
    apic.sw_enable();
    apic.set_spurious_vector(u8::MAX);
    apic.write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
    apic.write_register(Register::TimerInitialCount, u32::MAX);
    apic.timer().set_mode(TimerMode::OneShot);

    const MIN_FREQ: u32 = 1000;
    const FREQ_WINDOW: u64 = (MIN_FREQ / 100) as u64;

    pic8259::enable();
    idt::set_interrupt_handler(32, pit_tick_handler);
    pic8259::pit::set_timer_freq(MIN_FREQ, pic8259::pit::OperatingMode::RateGenerator);

    trace!("Determining APIT frequency using PIT windowing.");
    unsafe { MSR::IA32_GS_BASE.write(0) };
    apic.timer().set_masked(false);
    libstd::instructions::interrupts::enable();
    while MSR::IA32_GS_BASE.read() < FREQ_WINDOW {}
    apic.timer().set_masked(true);
    unsafe { MSR::IA32_GS_BASE.write(0) };

    trace!("Disabling 8259 emulated PIC.");
    libstd::instructions::interrupts::without_interrupts(|| unsafe { pic8259::disable() });

    let mut per_ms =
        (u32::MAX - apic.read_register(Register::TimerCurrentCount)) / (FREQ_WINDOW as u32);
    per_ms *= u32::max((lpu.clock().frequency() as u32) / MIN_FREQ, 1);
    apic.write_register(Register::TimerInitialCount, per_ms);
    debug!(
        "APIC clock frequency: {}KHz",
        apic.read_register(Register::TimerInitialCount)
    );

    idt::set_interrupt_handler(32, drivers::clock::apic_tick_handler);
    apic.timer().set_vector(32);
    idt::set_interrupt_handler(58, apic_error_handler);
    apic.error().set_vector(58);

    apic.timer().set_mode(TimerMode::Periodic);
    apic.timer().set_masked(false);
    apic.sw_enable();

    debug!("Core-local APIC configured and enabled.");
}

extern "x86-interrupt" fn apic_error_handler(_: libstd::structures::idt::InterruptStackFrame) {
    let apic = &crate::lpu::try_get().unwrap().apic();

    error!("APIC ERROR INTERRUPT");
    error!("--------------------");
    error!("DUMPING APIC ERROR REGISTER:");
    error!("  {:?}", apic.error_status());

    apic.end_of_interrupt();
}
