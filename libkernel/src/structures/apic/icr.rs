use crate::{memory::volatile::VolatileCell, ReadWrite};

#[repr(u32)]
pub enum DeliveryMode {
    Fixed = 0,
    SMI = 2,
    NMI = 4,
    INIT = 5,
    SIPI = 6,
}

#[repr(u32)]
pub enum DestinationShorthand {
    None = 0b00,
    AllIncludingSelf = 0b10,
    AllExcludingSelf = 0b01,
}

#[repr(C)]
pub struct InterruptCommandRegister {
    low: VolatileCell<u32, ReadWrite>,
    high: VolatileCell<u32, ReadWrite>,
}

impl crate::memory::volatile::Volatile for InterruptCommandRegister {}

impl InterruptCommandRegister {
    pub fn send_init(&self, apic_id: u8) {
        self.send(
            0,
            DeliveryMode::INIT,
            false,
            true,
            DestinationShorthand::AllExcludingSelf,
            apic_id,
        );
    }

    pub fn send_sipi(&self, vector: u8, apic_id: u8) {
        self.send(
            vector,
            DeliveryMode::SIPI,
            false,
            false,
            DestinationShorthand::AllExcludingSelf,
            apic_id,
        );
    }

    pub fn send(
        &self,
        vector: u8,
        delivery_mode: DeliveryMode,
        logical_mode: bool,
        deassert: bool,
        dest_shorthand: DestinationShorthand,
        apic_id: u8,
    ) {
        assert!(
            !self.is_pending(),
            "Cannot send command when command is already pending."
        );

        self.high.write((apic_id as u32) << 24);
        self.low.write(
            (vector as u32)
                | ((delivery_mode as u32) << 8)
                | ((!logical_mode as u32) << 11)
                | ((!deassert as u32) << 14)
                | ((deassert as u32) << 15)
                | ((dest_shorthand as u32) << 18),
        );
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
