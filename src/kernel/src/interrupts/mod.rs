mod instructions;

pub use instructions::*;
use libkernel::{Address, Virtual};

use core::cell::SyncUnsafeCell;
use num_enum::TryFromPrimitive;

const PIC_BASE: u8 = 0xE0;

/// Delivery mode for IPIs.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum DeliveryMode {
    Fixed = 0b000,
    LowPriority = 0b001,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
    ExtINT = 0b111,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DestinationMode {
    Physical = 0,
    Logical = 1,
}

#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
#[allow(non_camel_case_types)]
pub enum Vector {
    Clock = 0x20,
    /* 0x21..=0x2F reserved for PIC */
    Timer = 0x30,
    Thermal = 0x32,
    Performance = 0x33,
    /* 0x34..=0x3B free for use */
    Error = 0x3C,
    LINT0 = 0x3D,
    LINT1 = 0x3E,
    SPURIOUS = 0x3F,
}

#[derive(Debug, Clone, Copy)]
pub struct ControlFlowContext {
    pub ip: u64,
    pub sp: u64,
}

/// Indicates what type of error the common page fault handler encountered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageFaultHandlerError {
    AddressNotMapped,
    NotDemandPaged,
    CriticalError,
}

/// SAFETY: This function expects only to be called upon a processor page fault exception.
pub unsafe fn common_page_fault_handler(address: Address<Virtual>) -> Result<(), PageFaultHandlerError> {
    use crate::memory;
    use libkernel::memory::Page;

    let fault_page = Page::from_address_contains(address);
    let hhdm_page = memory::get_kernel_hhdm_page();
    let page_manager = memory::PageManager::from_current(&hhdm_page);
    let Some(mut fault_page_attributes) = page_manager.get_page_attributes(&fault_page) else { return Err(PageFaultHandlerError::AddressNotMapped) };

    if fault_page_attributes.contains(memory::PageAttributes::DEMAND) {
        page_manager.auto_map(
            &fault_page,
            {
                // remove demand bit ...
                fault_page_attributes.remove(memory::PageAttributes::DEMAND);
                // ... insert usable RW bits ...
                fault_page_attributes.insert(memory::PageAttributes::RW);
                // ... return attributes
                fault_page_attributes
            },
            memory::get_kernel_frame_manager(),
        );

        // SAFETY: We know the page was just mapped, and contains no relevant memory.
        fault_page.clear_memory();

        Ok(())
    } else {
        Err(PageFaultHandlerError::NotDemandPaged)
    }
}

/* NON-EXCEPTION IRQ HANDLING */
#[cfg(target_arch = "x86_64")]
pub type ArchContext = (crate::arch::x64::cpu::GeneralContext, crate::arch::x64::cpu::SpecialContext);

type InterruptHandler = fn(u64, &mut ControlFlowContext, &mut ArchContext);
static INTERRUPT_HANDLER: SyncUnsafeCell<InterruptHandler> = SyncUnsafeCell::new(common_interrupt_handler);

/// Gets the current common interrupt handler.
#[inline]
pub fn get_common_interrupt_handler() -> &'static InterruptHandler {
    unsafe { &*INTERRUPT_HANDLER.get() }
}

pub fn common_interrupt_handler(
    irq_vector: u64,
    ctrl_flow_context: &mut ControlFlowContext,
    arch_context: &mut ArchContext,
) {
    match Vector::try_from(irq_vector) {
        Ok(vector) if vector == Vector::Timer => {
            crate::local_state::schedule_next_task(ctrl_flow_context, arch_context);
        }

        vector_result => {
            warn!("Unhandled IRQ vector: {:?}", vector_result);
        }
    }

    #[cfg(target_arch = "x86_64")]
    crate::arch::x64::structures::apic::end_of_interrupt();
}
