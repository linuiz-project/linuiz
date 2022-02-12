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
    const_refs_to_cell
)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate lib;

mod clock;
mod drivers;
mod local_state;
mod logging;
mod scheduling;
mod slob;
mod syscall;
mod tables;

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
    static __bss_end: LinkerSymbol;

    static __user_code_start: LinkerSymbol;
    static __user_code_end: LinkerSymbol;
}

static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);

#[export_name = "__ap_stack_pointers"]
static mut AP_STACK_POINTERS: [*const (); 256] = [core::ptr::null(); 256];

static KERNEL_PAGE_MANAGER: SyncOnceCell<PageManager> = SyncOnceCell::new();
static KERNEL_MALLOCATOR: SyncOnceCell<slob::SLOB> = SyncOnceCell::new();

/// Clears the kernel stack by resetting `RSP`.
///
/// SAFETY: This method does *extreme* damage to the stack. It should only ever be used when
///         ABSOLUTELY NO dangling references to the old stack will exist (i.e. calling a
///         no-argument non-returning function directly after).
macro_rules! reset_bsp_stack_ptr {
    () => {
        assert!(
            lib::cpu::is_bsp(),
            "Cannot clear AP stack pointers to BSP stack top."
        );

        // TODO implement shadow stacks (?) and research them

        lib::registers::stack::RSP::write(__bsp_stack.as_mut_ptr());
        // Serializing instruction to clear pipeline of any dangling references (and order all instructions before / after).
        lib::instructions::cpuid::exec(0x0, 0x0).unwrap();
    };
}

#[no_mangle]
unsafe extern "efiapi" fn _entry(
    boot_info: BootInfo<uefi::MemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    /* PRE-INIT (no environment prepared) */
    boot_info.validate_magic();
    if let Err(_) = lib::BOOT_INFO.set(boot_info) {
        panic!("`BOOT_INFO` already set.");
    }

    /* INIT STDOUT */
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);

    match drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => lib::instructions::interrupts::breakpoint(),
    }

    // Write misc. CPU state to stdout (This also lazy initializes them).
    {
        debug!("CPU Vendor          {:?}", lib::cpu::VENDOR);
        debug!("CPU Features        {:?}", lib::cpu::FEATURES);
        debug!("CPU Features Ext    {:?}", lib::cpu::FEATURES_EXT);
    }

    /* COMMON KERNEL START (prepare local state and AP processors) */
    reset_bsp_stack_ptr!();
    _startup()
}

fn load_registers() {
    unsafe {
        // Set CR0 flags.
        use lib::registers::control::{CR0Flags, CR0};
        CR0::write(
            CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG,
        );
        // Set CR4 flags.
        use lib::registers::control::{CR4Flags, CR4};
        CR4::write(
            CR4Flags::DE
                | CR4Flags::PAE
                | CR4Flags::MCE
                | CR4Flags::PGE
                | CR4Flags::OSFXSR
                | CR4Flags::OSXMMEXCPT,
        );
        // Enable use of the `NO_EXECUTE` page attribute, if supported.
        if lib::cpu::FEATURES_EXT.contains(lib::cpu::FeaturesExt::NO_EXEC) {
            lib::registers::msr::IA32_EFER::set_nxe(true);
        }
    }
}

fn load_tables() {
    use tables::{gdt, idt};

    // Always initialize GDT prior to configuring IDT.
    gdt::init();

    if lib::cpu::is_bsp() {
        // Due to the fashion in which the x86_64 crate initializes the IDT entries,
        // it must be ensured that the handlers are set only *after* the GDT has been
        // properly initialized and loaded—otherwise, the `CS` value for the IDT entries
        // is incorrect, and this causes very confusing GPFs.
        idt::init();
    }

    crate::tables::idt::load();
}

fn load_tss() {
    use core::num::NonZeroUsize;
    use lib::memory::malloc;
    use x86_64::{
        instructions::tables,
        structures::{
            gdt::{Descriptor, GlobalDescriptorTable},
            tss::TaskStateSegment,
        },
    };

    unsafe {
        let tss_ptr = malloc::get()
            .alloc(
                core::mem::size_of::<TaskStateSegment>(),
                NonZeroUsize::new(core::mem::align_of::<TaskStateSegment>()),
            )
            .unwrap()
            .cast::<TaskStateSegment>()
            .unwrap()
            .into_parts()
            .0;

        use x86_64::VirtAddr;
        tss_ptr.as_mut().unwrap().interrupt_stack_table[0] =
            VirtAddr::from_ptr(alloc_stack(1, false));
        tss_ptr.as_mut().unwrap().interrupt_stack_table
            [crate::tables::idt::DOUBLE_FAULT_IST_INDEX as usize] =
            VirtAddr::from_ptr(alloc_stack(1, false));

        let tss_descriptor = {
            use bit_field::BitField;

            let tss_ptr_u64 = tss_ptr as u64;

            let mut low = x86_64::structures::gdt::DescriptorFlags::PRESENT.bits();
            // base
            low.set_bits(16..40, tss_ptr_u64.get_bits(0..24));
            low.set_bits(56..64, tss_ptr_u64.get_bits(24..32));
            // limit (the `-1` in needed since the bound is inclusive)
            low.set_bits(0..16, (core::mem::size_of::<TaskStateSegment>() - 1) as u64);
            // type (0b1001 = available 64-bit tss)
            low.set_bits(40..44, 0b1001);

            let mut high = 0;
            high.set_bits(0..32, tss_ptr_u64.get_bits(32..64));

            Descriptor::SystemSegment(low, high)
        };

        let cur_gdt = tables::sgdt();
        let mut temp_gdt = GlobalDescriptorTable::new();
        temp_gdt.add_entry(Descriptor::kernel_code_segment());
        temp_gdt.add_entry(Descriptor::kernel_data_segment());
        let tss_selector = temp_gdt.add_entry(tss_descriptor);
        temp_gdt.load_unsafe();

        // Load TSS from temporary GDT.
        tables::load_tss(tss_selector);
        // Restore cached GDT.
        tables::lgdt(&cur_gdt);
    }
}

unsafe fn init_memory() {
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
            use lib::memory::PageAttributes;

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

/// This method assumes the `gs` segment has a valid base for kernel local state.
unsafe fn wake_aps() {
    use lib::acpi::rdsp::xsdt::{
        madt::{InterruptDevice, MADT},
        XSDT,
    };

    let lapic_id = crate::local_state::id() as u8 /* possibly don't cast to u8? */;
    let icr = crate::local_state::int_ctrl().icr();
    let ap_text_page_index = (__ap_text_start.as_usize() / 0x1000) as u8;

    if let Some(madt) = XSDT.find_sub_table::<MADT>() {
        info!("Beginning wake-up sequence for enabled processors.");
        for interrupt_device in madt.iter() {
            // Filter out non-lapic devices.
            if let InterruptDevice::LocalAPIC(ap_lapic) = interrupt_device {
                use lib::acpi::rdsp::xsdt::madt::LocalAPICFlags;
                // Filter out invalid lapic devices.
                if lapic_id != ap_lapic.id()
                    && ap_lapic.flags().intersects(
                        LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                    )
                {
                    debug!("Waking core ID {}.", ap_lapic.id());

                    AP_STACK_POINTERS[ap_lapic.id() as usize] = alloc_stack(1, false);

                    // Reset target processor.
                    trace!("Sending INIT interrupt to: {}", ap_lapic.id());
                    icr.send_init(ap_lapic.id());
                    icr.wait_pending();
                    // REMARK: IA32 spec indicates that doing this twice, as so, ensures the interrupt is received.
                    trace!("Sending SIPI x1 interrupt to: {}", ap_lapic.id());
                    icr.send_sipi(ap_text_page_index, ap_lapic.id());
                    icr.wait_pending();
                    trace!("Sending SIPI x2 interrupt to: {}", ap_lapic.id());
                    icr.send_sipi(ap_text_page_index, ap_lapic.id());
                    icr.wait_pending();
                }
            }
        }
    }

    // At this point, none of the APs have a stack, so they will wait at the beginning of _startup for memory to initialize and stacks to be doled out.
}

#[no_mangle]
unsafe extern "win64" fn _startup() -> ! {
    use lib::cpu::is_bsp;

    load_registers();
    load_tables();

    if is_bsp() {
        init_memory();
    }

    load_tss();

    // Initialize the processor-local state (always before waking APs, for access to ICR).
    local_state::init();
    local_state::enable();

    {
        let int_ctrl = local_state::int_ctrl();
        int_ctrl.sw_enable();
        int_ctrl.reload_timer(core::num::NonZeroU32::new(1));
    }

    if is_bsp() {
        wake_aps();
    }

    use crate::tables::gdt;
    use lib::registers::msr;

    // Enable `syscall`/`sysret`.
    msr::IA32_EFER::set_sce(true);
    // Configure system call environment registers.
    msr::IA32_STAR::set_selectors(
        *gdt::KCODE_SELECTOR.get().unwrap(),
        *gdt::KDATA_SELECTOR.get().unwrap(),
    );
    msr::IA32_LSTAR::set_syscall(syscall::syscall_enter);
    msr::IA32_SFMASK::set_rflags_mask(lib::registers::RFlags::all());

    local_state::disable();

    if is_bsp() {
        lib::registers::stack::RSP::write(alloc_stack(2, true));
        lib::cpu::ring3_enter(test_user_function, lib::registers::RFlags::INTERRUPT_FLAG);
    }

    kernel_main()
}

fn alloc_stack(pages: usize, is_userspace: bool) -> *mut () {
    unsafe {
        let (stack_bottom, stack_len) = lib::memory::malloc::get()
            .alloc_pages(pages)
            .unwrap()
            .1
            .into_parts();
        let stack_top = stack_bottom.add(stack_len);

        {
            use lib::memory::{AttributeModify, Page, PageAttributes};

            for page in Page::range(
                (stack_bottom as usize) / 0x1000,
                (stack_top as usize) / 0x1000,
            ) {
                lib::memory::get_page_manager().set_page_attribs(
                    &page,
                    PageAttributes::PRESENT
                        | PageAttributes::WRITABLE
                        | PageAttributes::NO_EXECUTE
                        | if is_userspace {
                            PageAttributes::USERSPACE
                        } else {
                            PageAttributes::empty()
                        },
                    AttributeModify::Set,
                );
            }
        }

        stack_top as *mut ()
    }
}

fn kernel_main() -> ! {
    debug!("Successfully entered `kernel_main()`.");

    lib::instructions::hlt_indefinite()
}

#[link_section = ".user_code"]
fn test_user_function() {
    unsafe {
        core::arch::asm!(
            "mov r10, $0",
            // "mov r8,   0x1F1F1FA1",
            // "mov r9,   0x1F1F1FA2",
            // "mov r13,   0x1F1F1FA3",
            // "mov r14,   0x1F1F1FA4",
            // "mov r15,   0x1F1F1FA5",
            "syscall",
            out("rcx") _,
            out("rdx") _,
            out("r10") _,
            out("r11") _,
            out("r12") _,
        )
    };

    loop {}
}
