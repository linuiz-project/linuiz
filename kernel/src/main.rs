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
extern crate libkernel;

mod clock;
mod drivers;
mod local_state;
mod logging;
mod memory;
mod scheduling;
mod slob;
mod syscall;
mod tables;

use alloc::vec::Vec;
use libkernel::{acpi::SystemConfigTableEntry, memory::uefi, BootInfo, LinkerSymbol};

extern "C" {
    static __bsp_stack: LinkerSymbol;
}

static mut CON_OUT: drivers::stdout::Serial = drivers::stdout::Serial::new(drivers::stdout::COM1);

#[export_name = "__ap_stack_pointers"]
static mut AP_STACK_POINTERS: [*const (); 256] = [core::ptr::null(); 256];

use libkernel::io::pci;
pub struct Devices<'a>(Vec<pci::DeviceVariant>, &'a core::marker::PhantomData<()>);
unsafe impl Send for Devices<'_> {}
unsafe impl Sync for Devices<'_> {}

impl Devices<'_> {
    pub fn iter(&self) -> core::slice::Iter<pci::DeviceVariant> {
        self.0.iter()
    }
}

lazy_static::lazy_static! {
    pub static ref PCIE_DEVICES: Devices<'static> =
        Devices(
            libkernel::io::pci::get_pcie_devices(Some(&*crate::memory::PAGE_MANAGER)).collect(),
            &core::marker::PhantomData
        );
}

/// Clears the kernel stack by resetting `RSP`.
///
/// SAFETY: This method does *extreme* damage to the stack. It should only ever be used when
///         ABSOLUTELY NO dangling references to the old stack will exist (i.e. calling a
///         no-argument non-returning function directly after).
macro_rules! reset_bsp_stack_ptr {
    () => {
        assert!(
            libkernel::cpu::is_bsp(),
            "Cannot clear AP stack pointers to BSP stack top."
        );

        // TODO implement shadow stacks (?) and research them

        libkernel::registers::stack::RSP::write(__bsp_stack.as_mut_ptr());
        // Serializing instruction to clear pipeline of any dangling references (and order all instructions before / after).
        libkernel::instructions::cpuid::exec(0x0, 0x0).unwrap();
    };
}

#[no_mangle]
unsafe extern "efiapi" fn _entry(
    boot_info: BootInfo<uefi::MemoryDescriptor, SystemConfigTableEntry>,
) -> ! {
    /* PRE-INIT (no environment prepared) */
    boot_info.validate_magic();
    if let Err(_) = libkernel::BOOT_INFO.set(boot_info) {
        panic!("`BOOT_INFO` already set.");
    }

    /* INIT STDOUT */
    CON_OUT.init(drivers::stdout::SerialSpeed::S115200);

    match drivers::stdout::set_stdout(&mut CON_OUT, log::LevelFilter::Debug) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
        }
        Err(_) => libkernel::instructions::interrupts::breakpoint(),
    }

    // Write misc. CPU state to stdout (This also lazy initializes them).
    {
        debug!("CPU Vendor          {}", libkernel::cpu::VENDOR);
        debug!("CPU Features        {:?}", libkernel::cpu::FeatureFmt);
    }

    /* COMMON KERNEL START (prepare local state and AP processors) */
    reset_bsp_stack_ptr!();
    _startup()
}

/// This method assumes the `gs` segment has a valid base for kernel local state.
unsafe fn wake_aps() {
    use libkernel::acpi::rdsp::xsdt::{
        madt::{InterruptDevice, MADT},
        XSDT,
    };

    let lapic_id = libkernel::cpu::get_id() as u8 /* possibly don't cast to u8? */;
    let icr = libkernel::structures::apic::APIC::interrupt_command_register();
    let ap_text_page_index = (memory::__ap_text_start.as_usize() / 0x1000) as u8;

    if let Some(madt) = XSDT.find_sub_table::<MADT>() {
        info!("Beginning wake-up sequence for enabled processors.");
        for interrupt_device in madt.iter() {
            // Filter out non-lapic devices.
            if let InterruptDevice::LocalAPIC(ap_lapic) = interrupt_device {
                use libkernel::acpi::rdsp::xsdt::madt::LocalAPICFlags;
                // Filter out invalid lapic devices.
                if lapic_id != ap_lapic.id()
                    && ap_lapic.flags().intersects(
                        LocalAPICFlags::PROCESSOR_ENABLED | LocalAPICFlags::ONLINE_CAPABLE,
                    )
                {
                    debug!("Waking core ID {}.", ap_lapic.id());

                    AP_STACK_POINTERS[ap_lapic.id() as usize] = alloc_stack(2, false);

                    info!("{:?}", AP_STACK_POINTERS[ap_lapic.id() as usize]);

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
}

#[no_mangle]
#[inline(never)]
unsafe extern "win64" fn _startup() -> ! {
    use libkernel::cpu::is_bsp;
    /* LOAD REGISTERS */
    {
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
        }
    }

    /* LOAD TABLES */
    {
        use tables::{gdt, idt};

        // Always initialize GDT prior to configuring IDT.
        gdt::init();

        if libkernel::cpu::is_bsp() {
            // Due to the fashion in which the x86_64 crate initializes the IDT entries,
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

    /* INIT MEMORY */
    if is_bsp() {
        use libkernel::{
            memory::{
                malloc, AttributeModify, FrameError, FrameType, Page, PageAttributes, FRAME_MANAGER,
            },
            registers::msr::IA32_APIC_BASE,
        };

        // ... modify known frame types ...
        FRAME_MANAGER
            .try_modify_type(
                IA32_APIC_BASE::get_base_addr().frame_index(),
                FrameType::MMIO,
            )
            .ok();
        // ... initialize memory ...
        memory::init(libkernel::BOOT_INFO.get().unwrap().memory_map());
        // ... modify all page attributes ...
        crate::memory::PAGE_MANAGER.set_page_attribs(
            &Page::from_index(IA32_APIC_BASE::get_base_addr().frame_index()),
            PageAttributes::MMIO,
            AttributeModify::Set,
        );
    }

    local_state::init();
    local_state::enable();

    if is_bsp() {
        clock::global::start();
    }

    /* LOAD TSS */
    {
        use x86_64::{
            instructions::tables,
            structures::{
                gdt::{Descriptor, GlobalDescriptorTable},
                tss::TaskStateSegment,
            },
        };

        let tss_ptr = libkernel::alloc_obj!(TaskStateSegment);

        {
            use crate::local_state::Offset;
            use x86_64::VirtAddr;

            (&mut *tss_ptr).privilege_stack_table[0] =
                VirtAddr::from_ptr(crate::rdgsval!(*const (), Offset::PrivilegeStackPtr));
        }

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
        local_state::init_local_apic();
        // local_state::reload_timer(core::num::NonZeroU32::new(1));
    }
    if is_bsp() {
        wake_aps();
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

    libkernel::registers::stack::RSP::write(alloc_stack(1, true));
    libkernel::cpu::ring3_enter(test_user_function, libkernel::registers::RFlags::empty());

    debug!("Failed to enter ring 3.");

    libkernel::instructions::hlt_indefinite()
}

fn alloc_stack(pages: usize, is_userspace: bool) -> *mut () {
    unsafe {
        let (stack_bottom, stack_len) = libkernel::memory::malloc::get()
            .alloc_pages(pages)
            .unwrap()
            .1
            .into_parts();
        let stack_top = stack_bottom.add(stack_len);

        {
            use libkernel::memory::{AttributeModify, Page, PageAttributes};

            for page in Page::range(
                (stack_bottom as usize) / 0x1000,
                (stack_top as usize) / 0x1000,
            ) {
                memory::PAGE_MANAGER.set_page_attribs(
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
