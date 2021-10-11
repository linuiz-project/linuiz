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
    low: u32,
    high: u32,
}

impl InterruptCommandRegister {
    pub const fn init(apic_id: u8) -> Self {
        Self::new(
            0,
            DeliveryMode::INIT,
            false,
            true,
            DestinationShorthand::AllExcludingSelf,
            apic_id,
        )
    }

    pub const fn sipi(vector: u8, apic_id: u8) -> Self {
        Self::new(
            vector,
            DeliveryMode::SIPI,
            false,
            false,
            DestinationShorthand::AllExcludingSelf,
            apic_id,
        )
    }

    pub const fn new(
        vector: u8,
        delivery_mode: DeliveryMode,
        logical_mode: bool,
        deassert: bool,
        dest_shorthand: DestinationShorthand,
        apic_id: u8,
    ) -> Self {
        Self {
            low: (vector as u32)
                | ((delivery_mode as u32) << 8)
                | ((!logical_mode as u32) << 11)
                | ((!deassert as u32) << 14)
                | ((deassert as u32) << 15)
                | ((dest_shorthand as u32) << 18),
            high: (apic_id as u32) << 24,
        }
    }

    pub const fn low_bits(&self) -> u32 {
        self.low
    }

    pub const fn high_bits(&self) -> u32 {
        self.high
    }
}
