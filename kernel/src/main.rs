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
    asm_const
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate lib;

mod clock;
mod drivers;
mod gdt;
mod local_state;
mod logging;
mod scheduling;
mod slob;
mod syscall;

use lib::{
    acpi::SystemConfigTableEntry,
    cell::SyncOnceCell,
    memory::{uefi, PageManager},
    BootInfo, LinkerSymbol,
};

extern "C" {
    static __ap_text_start: LinkerSymbol;
    static __ap_text_end: LinkerSymbol;

    static __ap_data_start: LinkerSymbol;
    static __kernel_pml4: LinkerSymbol;
    static __ap_data_end: LinkerSymbol;

    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;

    static __rodata_start: LinkerSymbol;
    static __rodata_end: LinkerSymbol;

    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;

    static __bss_start: LinkerSymbol;
    static __bsp_stack: LinkerSymbol;
    static __exception_stack: LinkerSymbol;
    static __double_fault_stack: LinkerSymbol;
    static __isr_stack: LinkerSymbol;
    static __bss_end: LinkerSymbol;

    static __user_code_start: LinkerSymbol;
    static __user_code_end: LinkerSymbol;
}

#[export_name = "__ap_stack_pointers"]
static mut AP_STACK_POINTERS: [*const (); 256] = [core::ptr::null(); 256];
static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);
static KERNEL_PAGE_MANAGER: SyncOnceCell<PageManager> = SyncOnceCell::new();
static KERNEL_MALLOCATOR: SyncOnceCell<slob::SLOB> = SyncOnceCell::new();

/// Clears the kernel stack by resetting `RSP`.
///
/// SAFETY: This method does *extreme* damage to the stack. It should only ever be used when
///         ABSOLUTELY NO dangling references to the old stack will exist (i.e. calling a
///         no-argument non-returning function directly after).
macro_rules! clear_bsp_stack {
    () => {
        assert!(
            $crate::local_state::is_bsp(),
            "Cannot clear AP stack pointers to BSP stack top."
        );

        lib::registers::stack::RSP::write(__bsp_stack.as_mut_ptr());
        // Serializing instruction to clear pipeline of any dangling references (and order all instructions before / after).
        lib::instructions::cpuid::exec(0x0, 0x0).unwrap();
    };
}

#[no_mangle]
#[export_name = "_entry"]
unsafe extern "efiapi" fn kernel_init(
    boot_info: BootInfo<uefi::MemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    /* PRE-INIT (no environment prepared) */
    boot_info.validate_magic();
    if let Err(_) = lib::BOOT_INFO.set(boot_info) {
        panic!("`BOOT_INFO` already set.");
    }

    clear_bsp_stack!();

    /* INIT STDOUT */
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);

    match drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => lib::instructions::interrupts::breakpoint(),
    }

    // Set up TSS stacks.
    {
        use lib::structures::idt;

        lib::structures::gdt::TSS_STACK_PTRS[idt::EXCEPTION_IST_INDEX as usize] =
            Some(__exception_stack.as_mut_ptr());
        lib::structures::gdt::TSS_STACK_PTRS[idt::DOUBLE_FAULT_IST_INDEX as usize] =
            Some(__double_fault_stack.as_mut_ptr());
        lib::structures::gdt::TSS_STACK_PTRS[idt::ISR_IST_INDEX as usize] =
            Some(__isr_stack.as_mut_ptr());
    }

    gdt::init();

    // Set CR0 flags.
    {
        use lib::registers::control::{CR0Flags, CR0};

        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
    }

    // Set CR4 flags.
    {
        use lib::registers::control::{CR4Flags, CR4};

        CR4::write(
            CR4Flags::DE
                | CR4Flags::PAE
                | CR4Flags::MCE
                | CR4Flags::PGE
                | CR4Flags::OSFXSR
                | CR4Flags::OSXMMEXCPT,
        );
    }

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if lib::cpu::FEATURES_EXT.contains(lib::cpu::FeaturesExt::NO_EXEC) {
        lib::registers::msr::IA32_EFER::set_nxe(true);
    }

    // Write misc. CPU state to stdout.
    {
        use lib::{cpu, registers::control};

        debug!("CPU Vendor          {:?}", cpu::VENDOR);
        debug!("CPU Features        {:?}", cpu::FEATURES);
        debug!("CPU Features Ext    {:?}", cpu::FEATURES_EXT);

        debug!("CR0:                {:?}", control::CR0::read());
        debug!("CR4:                {:?}", control::CR4::read());
    }

    /* INIT KERNEL MEMORY */
    {
        use lib::memory::Page;

        KERNEL_PAGE_MANAGER
            .set(PageManager::new(&Page::null()))
            .unwrap_or_else(|_| panic!(""));
        lib::memory::set_page_manager(KERNEL_PAGE_MANAGER.get().unwrap_or_else(|| panic!("")));
        KERNEL_MALLOCATOR
            .set(slob::SLOB::new())
            .unwrap_or_else(|_| panic!(""));

        // Configure and use page manager.
        {
            use lib::memory::{FrameType, FRAME_MANAGER};
            info!("Initializing kernel SLOB allocator.");

            {
                let page_manager = lib::memory::get_page_manager();

                debug!("Configuring page table entries for kernel ELF sections.");
                use lib::memory::{AttributeModify, PageAttributes};

                // Set page attributes for UEFI descriptor pages.
                for descriptor in lib::BOOT_INFO.get().unwrap().memory_map().iter() {
                    let mut page_attribs = PageAttributes::empty();

                    use lib::memory::uefi::{MemoryAttributes, MemoryType};

                    if descriptor.att.contains(MemoryAttributes::WRITE_THROUGH) {
                        page_attribs.insert(PageAttributes::WRITABLE);
                        page_attribs.insert(PageAttributes::WRITE_THROUGH);
                    }

                    if descriptor.att.contains(MemoryAttributes::WRITE_BACK) {
                        page_attribs.insert(PageAttributes::WRITABLE);
                        page_attribs.remove(PageAttributes::WRITE_THROUGH);
                    }

                    if descriptor.att.contains(MemoryAttributes::EXEC_PROTECT) {
                        page_attribs.insert(PageAttributes::NO_EXECUTE);
                    }

                    if descriptor.att.contains(MemoryAttributes::UNCACHEABLE) {
                        page_attribs.insert(PageAttributes::UNCACHEABLE);
                    }

                    if descriptor.att.contains(MemoryAttributes::READ_ONLY) {
                        page_attribs.remove(PageAttributes::WRITABLE);
                        page_attribs.remove(PageAttributes::WRITE_THROUGH);
                    }

                    // If the descriptor type is not unusable...
                    if !matches!(
                        descriptor.ty,
                        MemoryType::UNUSABLE
                            | MemoryType::UNACCEPTED
                            | MemoryType::KERNEL_CODE
                            | MemoryType::KERNEL_DATA
                    ) {
                        // ... then iterate its pages and identity map them.
                        //     This specific approach allows the memory usage to be decreased overall,
                        //      since unused/unusable pages or descriptors will not be mapped.
                        for page in descriptor
                            .frame_range()
                            .map(|index| Page::from_index(index))
                        {
                            page_manager
                                .identity_map(
                                    &page,
                                    PageAttributes::PRESENT | PageAttributes::GLOBAL | page_attribs,
                                )
                                .unwrap();
                        }
                    }
                }

                // Overwrite UEFI page attributes for kernel ELF sections.
                use lib::{align_down_div, align_up_div};
                let kernel_text = Page::range(
                    align_down_div(__text_start.as_usize(), 0x1000),
                    align_up_div(__text_end.as_usize(), 0x1000),
                );
                let kernel_rodata = Page::range(
                    align_down_div(__rodata_start.as_usize(), 0x1000),
                    align_up_div(__rodata_end.as_usize(), 0x1000),
                );
                let kernel_data = Page::range(
                    align_down_div(__data_start.as_usize(), 0x1000),
                    align_up_div(__data_end.as_usize(), 0x1000),
                );
                let kernel_bss = Page::range(
                    align_down_div(__bss_start.as_usize(), 0x1000),
                    align_up_div(__bss_end.as_usize(), 0x1000),
                );
                let ap_text = Page::range(
                    align_down_div(__ap_text_start.as_usize(), 0x1000),
                    align_up_div(__ap_text_end.as_usize(), 0x1000),
                );
                let ap_data = Page::range(
                    align_down_div(__ap_data_start.as_usize(), 0x1000),
                    align_up_div(__ap_data_end.as_usize(), 0x1000),
                );
                let user_code = Page::range(
                    align_down_div(__user_code_start.as_usize(), 0x1000),
                    align_up_div(__user_code_end.as_usize(), 0x1000),
                );

                for page in kernel_text.chain(ap_text) {
                    page_manager
                        .identity_map(&page, PageAttributes::PRESENT | PageAttributes::GLOBAL)
                        .unwrap();
                }

                for page in kernel_rodata {
                    page_manager
                        .identity_map(
                            &page,
                            PageAttributes::PRESENT
                                | PageAttributes::GLOBAL
                                | PageAttributes::NO_EXECUTE,
                        )
                        .unwrap();
                }

                for page in kernel_data.chain(kernel_bss).chain(ap_data).chain(
                    // Frame manager map frames/pages.
                    FRAME_MANAGER
                        .iter()
                        .enumerate()
                        .filter_map(|(frame_index, (ty, _, _))| {
                            if ty == FrameType::FrameMap {
                                Some(Page::from_index(frame_index))
                            } else {
                                None
                            }
                        }),
                ) {
                    page_manager
                        .identity_map(
                            &page,
                            PageAttributes::PRESENT
                                | PageAttributes::GLOBAL
                                | PageAttributes::NO_EXECUTE
                                | PageAttributes::WRITABLE,
                        )
                        .unwrap();
                }

                for page in user_code {
                    page_manager
                        .identity_map(&page, PageAttributes::PRESENT | PageAttributes::USERSPACE)
                        .unwrap();
                }

                // Since we're using physical offset mapping for our page table modification
                //  strategy, the memory needs to be identity mapped at the correct offset.
                let phys_mapping_addr = lib::memory::virtual_map_offset();
                debug!("Mapping physical memory at offset: {:?}", phys_mapping_addr);
                page_manager.modify_mapped_page(Page::from_addr(phys_mapping_addr));

                info!("Writing kernel addressor's PML4 to the CR3 register.");
                page_manager.write_cr3();
            }

            // Configure SLOB allocator.
            debug!("Allocating reserved physical memory frames...");
            let slob = KERNEL_MALLOCATOR.get().unwrap();
            FRAME_MANAGER
                .iter()
                .enumerate()
                .filter(|(_, (ty, _, _))| !matches!(ty, FrameType::Usable))
                .for_each(|(index, _)| {
                    slob.reserve_page(&Page::from_index(index)).unwrap();
                });

            info!("Finished block allocator initialization.");
        }

        debug!("Setting newly-configured default allocator.");
        lib::memory::malloc::set(KERNEL_MALLOCATOR.get().unwrap());
        // TODO somehow ensure the PML4 frame is within the first 32KiB for the AP trampoline
        debug!("Moving the kernel PML4 mapping frame into the global processor reference.");
        __kernel_pml4
            .as_mut_ptr::<u32>()
            .write(lib::registers::control::CR3::read().0.as_usize() as u32);

        info!("Kernel memory initialized.");
    }

    /* COMMON KERNEL START (prepare local state and AP processors) */
    clear_bsp_stack!();
    _startup()
}

#[no_mangle]
extern "C" fn _startup() -> ! {
    // Ensure we load the IDT as early as possible in startup sequence.
    unsafe { lib::structures::idt::load_unchecked() };

    if crate::local_state::is_bsp() {
        use lib::structures::idt;
        use local_state::{handlers, InterruptVector};

        // This is where we'll configure the kernel-static IDT entries.
        idt::set_handler_fn(InterruptVector::LocalTimer as u8, handlers::apit_handler);
        idt::set_handler_fn(InterruptVector::Storage as u8, handlers::storage_handler);
        idt::set_handler_fn(InterruptVector::Spurious as u8, handlers::spurious_handler);
        idt::set_handler_fn(InterruptVector::Error as u8, handlers::error_handler);

        // Initialize global clock (PIT).
        crate::clock::global::init();
    }

    // Initialize the processor-local state.
    crate::local_state::init();

    // If this is the BSP, wake other cores.
    if crate::local_state::is_bsp() {
        use lib::acpi::rdsp::xsdt::{
            madt::{InterruptDevice, MADT},
            XSDT,
        };

        // Initialize other CPUs
        let id = crate::local_state::processor_id();
        let icr = crate::local_state::int_ctrl().icr();
        let ap_text_page_index = unsafe { __ap_text_start.as_usize() / 0x1000 } as u8;

        if let Some(madt) = XSDT.find_sub_table::<MADT>() {
            info!("Beginning wake-up sequence for enabled processors.");
            for lapic in madt
                .iter()
                // Filter out non-lapic devices.
                .filter_map(|interrupt_device| {
                    if let InterruptDevice::LocalAPIC(apic_other) = interrupt_device {
                        Some(apic_other)
                    } else {
                        None
                    }
                })
                // Filter out invalid lapic devices.
                .filter(|lapic| {
                    use lib::acpi::rdsp::xsdt::madt::LocalAPICFlags;

                    lapic.id() != id
                        && lapic.flags().intersects(
                            LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                        )
                })
            {
                unsafe {
                    const AP_STACK_SIZE: usize = 0x2000;

                    let (stack_bottom, len) = lib::memory::malloc::get()
                        .alloc(AP_STACK_SIZE, core::num::NonZeroUsize::new(0x1000))
                        .unwrap()
                        .into_parts();

                    AP_STACK_POINTERS[lapic.id() as usize] = stack_bottom.add(len) as *mut _;
                };

                // Reset target processor.
                trace!("Sending INIT interrupt to: {}", lapic.id());
                icr.send_init(lapic.id());
                icr.wait_pending();
                // REMARK: IA32 spec indicates that doing this twice, as so, ensures the interrupt is received.
                trace!("Sending SIPI x1 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_text_page_index, lapic.id());
                icr.wait_pending();
                trace!("Sending SIPI x2 interrupt to: {}", lapic.id());
                icr.send_sipi(ap_text_page_index, lapic.id());
                icr.wait_pending();
            }
        }

        //     use lib::io::pci;
        //     let nvme_driver = pci::BRIDGES
        //         .lock()
        //         .iter()
        //         .flat_map(|bridge| bridge.iter())
        //         .flat_map(|bus| bus.iter())
        //         .find_map(|device| {
        //             if let pci::DeviceVariant::Standard(device) = device_variant {
        //                 if device.class() == pci::DeviceClass::MassStorageController
        //                     && device.subclass() == 0x08
        //                 {
        //                     Some(drivers::nvme::Controller::from_device(&device, 4, 4))
        //                 }
        //             }
        //         });

        //     use scheduling::Task;
        //     let mut thread = local_state::lock_thread();
        //     let mut nvme = nvme_driver.expect("No NVMe drive detected.");
        //     thread.push_task(Task::new(128, nvme.run, None, None));
        //     thread.push_task(Task::new(0, bsp_main, None, None));
        //     thread.set_enabled(true);
        // }

        // use scheduling::Thread;
        // let mut thread = local_state::lock_scheduler();
        // thread.set_enabled(true);
        // thread.push_thread(Thread::new(255, task1, None, None));
        // thread.push_thread(Thread::new(255, task2, None, None));
    }

    unsafe {
        use lib::registers::{msr, msr::Generic};

        // Enable `syscall`/`sysret`.
        msr::IA32_EFER::set_sce(true);
        // Configure system call environment registers.
        msr::IA32_STAR::set_selectors(
            *gdt::KCODE_SELECTOR.get().unwrap(),
            *gdt::KDATA_SELECTOR.get().unwrap(),
        );
        msr::IA32_LSTAR::set_syscall(syscall::syscall_enter);
        msr::IA32_SFMASK::set_rflags_mask(lib::registers::RFlags::all());
    }

    kernel_main()
}

#[link_section = ".user_code"]
fn test_user_function() {
    unsafe {
        core::arch::asm!(
            "mov r10, $0",
            "mov r8,   0x1F1F1FA1",
            "mov r9,   0x1F1F1FA2",
            "mov r13,   0x1F1F1FA3",
            "mov r14,   0x1F1F1FA4",
            "mov r15,   0x1F1F1FA5",
            "syscall",
            out("r10") _,
            options(nostack, nomem)
        )
    };
    loop {}
}

fn task1() -> ! {
    loop {
        for i in 65..91 {
            unsafe { CON_OUT.write(i) };
            clock::global::busy_wait_msec(1);
        }
    }
}

fn task2() -> ! {
    loop {
        for i in 97..123 {
            unsafe { CON_OUT.write(i) };
            clock::global::busy_wait_msec(1);
        }
    }
}

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    unsafe { lib::cpu::ring3_enter(test_user_function, lib::registers::RFlags::INTERRUPT_FLAG) };

    lib::instructions::hlt_indefinite()
}
