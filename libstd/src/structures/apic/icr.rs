use crate::{memory::volatile::VolatileCell, ReadWrite};

#[repr(u32)]
pub enum DeliveryMode {
    Fixed = 0b000,
    SMI = 0b010,
    NMI = 0b100,
    INIT = 0b101,
    StartUp = 0b110,
}

#[repr(u32)]
pub enum DestinationMode {
    Physical = 0,
    Logical = 1,
}

#[repr(u32)]
pub enum DestinationShorthand {
    None = 0b00,
    AllIncludingSelf = 0b10,
    AllExcludingSelf = 0b01,
}

#[repr(C)]
pub struct InterruptCommandRegister<'v> {
    low: &'v VolatileCell<u32, ReadWrite>,
    high: &'v VolatileCell<u32, ReadWrite>,
}

impl<'v> InterruptCommandRegister<'v> {
    pub(super) const fn new(
        low: &'v VolatileCell<u32, ReadWrite>,
        high: &'v VolatileCell<u32, ReadWrite>,
    ) -> Self {
        Self { low, high }
    }

    pub fn send_init(&self, apic_id: u8) {
        self.send(
            0,
            DeliveryMode::INIT,
            DestinationMode::Physical,
            true,
            DestinationShorthand::None,
            apic_id,
        );
    }

    pub fn send_sipi(&self, vector: u8, apic_id: u8) {
        self.send(
            vector,
            DeliveryMode::StartUp,
            DestinationMode::Physical,
            true,
            DestinationShorthand::None,
            apic_id,
        );
    }

    pub fn send(
        &self,
        vector: u8,
        delivery_mode: DeliveryMode,
        dest_mode: DestinationMode,
        deassert: bool,
        dest_shorthand: DestinationShorthand,
        apic_id: u8,
    ) {
        assert!(apic_id < 0b10000, "APIC ID must be no more than 4 bits.");
        assert!(
            !self.is_pending(),
            "Cannot send command when command is already pending."
        );

        let high = (apic_id as u32) << 24;
        let low = (vector as u32)
            | ((delivery_mode as u32) << 8)
            | ((dest_mode as u32) << 11)
            | ((deassert as u32) << 14)
            | ((dest_shorthand as u32) << 18);

        self.high.write(high);
        self.low.write(low);
    }

    pub fn is_pending(&self) -> bool {
        use bit_field::BitField;
        self.low.read().get_bit(12)
    }

    pub fn wait_pending(&self) {
        while self.is_pending() {
            crate::instructions::hlt();
        }
    }
}
