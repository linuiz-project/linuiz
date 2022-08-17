use alloc::vec::Vec;
use libkernel::memory::volatile::VolatileCell;
use spin::{Mutex, Once};

pub struct IoApic<'vol> {
    id: u8,
    version: u8,
    handled_irqs: core::ops::Range<u8>,
    ioregs: Mutex<(&'vol VolatileCell<u32, libkernel::WriteOnly>, &'vol VolatileCell<u32, libkernel::ReadWrite>)>,
}

unsafe impl Send for IoApic<'_> {}
unsafe impl Sync for IoApic<'_> {}

impl IoApic<'_> {
    #[inline(always)]
    pub const fn get_id(&self) -> u8 {
        self.id
    }

    #[inline(always)]
    pub const fn get_version(&self) -> u8 {
        self.version
    }

    pub fn handled_irqs(&self) -> core::ops::Range<u8> {
        self.handled_irqs.clone()
    }
}

static IOAPICS: Once<Vec<IoApic>> = Once::new();
/// Queries the platform for I/O APICs, and returns them in a collection.
pub fn get_io_apics() -> &'static Vec<IoApic<'static>> {
    IOAPICS.call_once(|| {
        crate::tables::acpi::get_apic_model()
            .io_apics
            .iter()
            .map(|ioapic_info| unsafe {
                use bit_field::BitField;

                let ptr =
                    ((ioapic_info.address as usize) + crate::memory::get_kernel_hhdm_addr().as_usize()) as *mut u32;
                assert!(ptr.is_aligned(), "I/O APIC pointers must be aligned");

                let ioregsel = &*ptr.cast::<VolatileCell<u32, libkernel::WriteOnly>>();
                let ioregwin = &*ptr.add(4).cast::<VolatileCell<u32, libkernel::ReadWrite>>();

                let id = {
                    ioregsel.write(0x0);
                    ioregwin.read().get_bits(24..28) as u8
                };
                let (version, irq_count) = {
                    ioregsel.write(0x1);
                    let value = ioregwin.read();
                    (value.get_bits(0..8) as u8, value.get_bits(16..24) as u8)
                };
                let irq_base = ioapic_info.global_system_interrupt_base as u8;
                let handled_irqs = irq_base..(irq_base + irq_count);

                IoApic { id, version, handled_irqs, ioregs: Mutex::new((ioregsel, ioregwin)) }
            })
            .collect()
    })
}
