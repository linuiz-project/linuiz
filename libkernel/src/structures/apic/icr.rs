use crate::{memory::volatile::VolatileCell, InterruptDeliveryMode, ReadWrite};

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
    pub const unsafe fn new(
        low: &'v VolatileCell<u32, ReadWrite>,
        high: &'v VolatileCell<u32, ReadWrite>,
    ) -> Self {
        Self { low, high }
    }

    /// Send the INIT IPI sequence to the specified processor.
    /// 
    /// SAFETY: It appears that on some models of CPUs, an INIT sequence will hard
    ///         reset the processor. This is obviously undesirable in most cases, so
    ///         it is advised to ensure that the INIT sequence is only ever sent to
    ///         each core a single time.
    pub unsafe fn send_init(&self, apic_id: u8) {
        self.send(
            0,
            InterruptDeliveryMode::INIT,
            DestinationMode::Physical,
            true,
            DestinationShorthand::None,
            apic_id,
        );
    }

    /// Send the Startup IPI to the specified core, with the specified vector.
    /// 
    /// SAFETY: It should be mentioned that, given the behaviour of multiple INIT IPIs,
    ///         processors seem to simply ignore multiple SIPIs. So, it is seemingly safe
    ///         to accidentally issue extra SIPI IPIs.
    pub fn send_sipi(&self, vector: u8, apic_id: u8) {
        self.send(
            vector,
            InterruptDeliveryMode::StartUp,
            DestinationMode::Physical,
            true,
            DestinationShorthand::None,
            apic_id,
        );
    }

    pub fn send(
        &self,
        vector: u8,
        delivery_mode: InterruptDeliveryMode,
        dest_mode: DestinationMode,
        deassert: bool,
        dest_shorthand: DestinationShorthand,
        apic_id: u8,
    ) {
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
        while self.is_pending() {}
    }
}
