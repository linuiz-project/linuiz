use crate::interrupts;
use acpi::platform::interrupt::{Polarity, TriggerMode};
// use alloc::vec::Vec;
use bit_field::BitField;
use libkernel::mem::VolatileCell;
use spin::Mutex;

#[repr(transparent)]
pub struct RedirectionEntry(u64);

impl RedirectionEntry {
    pub fn get_vector(&self) -> u8 {
        self.0.get_bits(0..8).try_into().unwrap()
    }

    pub fn set_vector(&mut self, vector: u8) {
        // TODO InterruptVector type for 32..256 vector checking?
        assert!((32..=255).contains(&vector), "provided vector must be within 32..256");

        self.0.set_bits(0..8, vector.into());
    }

    pub fn get_delivery_mode(&self) -> interrupts::DeliveryMode {
        interrupts::DeliveryMode::try_from(u8::try_from(self.0.get_bits(8..11)).unwrap())
            .expect("unexpectedly failed to convert interrupt delivery mode")
    }

    pub fn set_delivery_mode(&mut self, mode: interrupts::DeliveryMode) {
        self.0.set_bits(8..11, mode as u64);
    }

    pub fn get_destination_mode(&self) -> interrupts::DestinationMode {
        if self.0.get_bit(11) { interrupts::DestinationMode::Physical } else { interrupts::DestinationMode::Logical }
    }

    pub fn set_destination_mode(&mut self, dest_mode: interrupts::DestinationMode) {
        self.0.set_bit(11, (dest_mode as u64) > 0);
    }

    pub fn is_awaiting_delivery(&self) -> bool {
        self.0.get_bit(12)
    }

    pub fn get_pin_polarity(&self) -> Polarity {
        if self.0.get_bit(13) { Polarity::ActiveLow } else { Polarity::ActiveHigh }
    }

    pub fn set_pin_polarity(&mut self, polarity: Polarity) {
        self.0.set_bit(
            13,
            match polarity {
                Polarity::SameAsBus | Polarity::ActiveHigh => false,
                Polarity::ActiveLow => true,
            },
        );
    }

    pub fn get_trigger_mode(&self) -> TriggerMode {
        if self.0.get_bit(15) { TriggerMode::Edge } else { TriggerMode::Level }
    }

    pub fn set_trigger_mode(&mut self, trigger_mode: TriggerMode) {
        self.0.set_bit(
            15,
            match trigger_mode {
                TriggerMode::SameAsBus | TriggerMode::Edge => false,
                TriggerMode::Level => true,
            },
        );
    }

    pub fn get_masked(&self) -> bool {
        self.0.get_bit(16)
    }

    pub fn set_masked(&mut self, mask: bool) {
        self.0.set_bit(16, mask);
    }

    pub fn get_destination_id(&self) -> u8 {
        self.0.get_bits(56..64).try_into().unwrap()
    }

    pub fn set_destination_id(&mut self, dest_id: u8) {
        self.0.set_bits(56..64, dest_id.into());
    }
}

type IoApicRegisters<'a> =
    Mutex<(&'a VolatileCell<u32, libkernel::WriteOnly>, &'a VolatileCell<u32, libkernel::ReadWrite>)>;

pub struct IoApic<'a> {
    id: u8,
    version: u8,
    handled_irqs: core::ops::RangeInclusive<u32>,
    ioregs: IoApicRegisters<'a>,
}

// Safety: Non-read-only mutations are behind a [`spin::Mutex`].
unsafe impl Send for IoApic<'_> {}
// Safety: Non-read-only mutations are behind a [`spin::Mutex`].
unsafe impl Sync for IoApic<'_> {}

impl IoApic<'_> {
    #[inline]
    pub const fn get_id(&self) -> u8 {
        self.id
    }

    #[inline]
    pub const fn get_version(&self) -> u8 {
        self.version
    }

    pub fn handled_irqs(&self) -> core::ops::RangeInclusive<u32> {
        self.handled_irqs.clone()
    }

    pub fn get_redirection(&self, global_irq_num: u32) -> RedirectionEntry {
        assert!(self.handled_irqs().contains(&global_irq_num), "I/O APIC does not handle the provided redirection");

        let reg_base_index = 0x10 + (global_irq_num * 2);

        let ioregs = self.ioregs.lock();

        ioregs.0.write(reg_base_index);
        let low_bits = ioregs.1.read();
        ioregs.0.write(reg_base_index + 1);
        let high_bits = ioregs.1.read();

        RedirectionEntry((u64::from(high_bits) << 32) | u64::from(low_bits))
    }

    pub fn set_redirection(&self, global_irq_num: u32, redirection: &RedirectionEntry) {
        assert!(self.handled_irqs().contains(&global_irq_num), "I/O APIC does not handle the provided redirection");

        #[allow(clippy::cast_possible_truncation)]
        {
            let redirection_low = redirection.0 as u32;
            let redirection_high = (redirection.0 >> 32) as u32;
            let reg_base_index = 0x10 + (global_irq_num * 2);

            let ioregs = self.ioregs.lock();

            ioregs.0.write(reg_base_index);
            ioregs.1.write(redirection_low);
            ioregs.0.write(reg_base_index + 1);
            ioregs.1.write(redirection_high);
        }
    }

    pub fn modify_redirection(&self, global_irq_num: u32) {
        assert!(self.handled_irqs().contains(&global_irq_num), "I/O APIC does not handle the provided redirection");
    }
}

// TODO We don't need to store this probably, find some way to init architecture-specifically.
//      Maybe just iterate them once, processing redirections within the same context.
// static IOAPICS: Once<Vec<IoApic>> = Once::new();
// /// Queries the platform for I/O APICs, and returns them in a collection.
// pub fn get_io_apics() -> &'static Vec<IoApic<'static>> {
//     IOAPICS.call_once(|| {
//         todo!()

//          let platform_info = libsys::acpi::get_platform_info();

//          if let acpi::platform::interrupt::InterruptModel::Apic(apic) = &platform_info.interrupt_model {
//              apic.io_apics
//                  .iter()
//                  // TODO unsafety comment
//                  .map(|ioapic_info| unsafe {

//                      let (ioregsel, ioregwin) = {
//                          let Ok(ioapic_regs) = libsys::memory::get().allocate_to(Address::<Frame>::new_truncate(ioapic_info.address as u64), 1)
//                              else { panic!("failed to initialize I/O APIC") };

//                          (
//                              &*ioapic_regs.as_ptr::<VolatileCell<u32, libsys::WriteOnly>>(),
//                              &*ioapic_regs.as_ptr::<VolatileCell<u32, libsys::ReadWrite>>().add(1)
//                          )
//                      };

//                      let id = {
//                          ioregsel.write(0x0);
//                          ioregwin.read().get_bits(24..28) as u8
//                      };
//                      let (version, irq_count) = {
//                          ioregsel.write(0x1);
//                          let value = ioregwin.read();
//                          (value.get_bits(0..8) as u8, value.get_bits(16..24) as u32)
//                      };
//                      let irq_base = ioapic_info.global_system_interrupt_base;
//                      let handled_irqs = irq_base..=(irq_base + irq_count);

//                      IoApic { id, version, handled_irqs, ioregs: Mutex::new((ioregsel, ioregwin)) }
//                  })
//                  .collect()
//          } else {
//              alloc::vec::Vec::new()
//          }
//     })
// }
