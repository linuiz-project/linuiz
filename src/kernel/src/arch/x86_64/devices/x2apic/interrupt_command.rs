use crate::arch::x86_64::devices::x2apic::InterruptDeliveryMode;
use bit_field::BitField;
use core::num::NonZeroU8;

/// Specifies the destination mode of an interrupt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDestinationMode {
    /// In physical destination mode, the destination processor is specified by its local APIC ID. For Pentium 4
    /// and Intel Xeon processors, either a single destination (local APIC IDs 0x00 through 0xFE) or a broadcast
    /// to all APICs (the APIC ID is 0xFF) may be specified in physical destination mode. A broadcast inter-process
    /// interrupt (bits 28-31 of the message destination address are 1) or I/O subsystem initiated interrupt with
    /// lowest priority delivery mode is not supported in physical destination mode and must not be configured by
    /// software. Also, for any non-broadcast inter-process interrupt or I/O subsystem initiated interrupt with
    /// lowest priority delivery mode, software must ensure that APICs defined in the interrupt address are present
    /// and enabled to receive interrupts. For the P6 family and Pentium processors, a single destination is
    /// specified in physical destination mode with a local APIC ID of 0x00 through 0x0E, allowing up to 15 local
    /// APICs to be addressed on the APIC bus. A broadcast to all local APICs is specified with 0x0F.
    ///
    /// Note: The number of local APICs that can be addressed on the system bus may be restricted by hardware.
    Physical,

    /// In logical destination mode, inter-process interrupt destination is specified using an 8-bit message destination
    /// address, which is entered in the destination field of the interrupt command register. Upon receiving an inter-process
    /// interrupt message that was sent using logical destination mode, a local APIC compares the message destination address
    /// in the message with the values in its logical destination register and destination format register to determine if it
    /// should accept and handle the inter-process interrupt. For both configurations of logical destination mode, when combined
    /// with lowest priority delivery mode, software is responsible for ensuring that all of the local APICs included in or
    /// addressed by the inter-process interrupt or I/O subsystem interrupt are present and enabled to receive the interrupt.
    ///
    /// Note: The logical APIC ID should not be confused with the local APIC ID that is contained in the local APIC
    ///       ID register.
    Logical,
}

impl From<InterruptDestinationMode> for bool {
    fn from(value: InterruptDestinationMode) -> Self {
        match value {
            InterruptDestinationMode::Physical => false,
            InterruptDestinationMode::Logical => true,
        }
    }
}

/// Specifies an interrupt trigger mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptTriggerMode {
    Edge,
    Level,
}

impl From<InterruptTriggerMode> for bool {
    fn from(value: InterruptTriggerMode) -> Self {
        match value {
            InterruptTriggerMode::Edge => false,
            InterruptTriggerMode::Level => true,
        }
    }
}

/// Specifies an interrupt level assertion.
///
/// For the INIT level de-assert delivery mode this flag must be set to 0; for all other delivery
/// modes it must be set to 1. (This flag has no meaning in Pentium 4 and Intel Xeon processors,
/// and will always be issued as a 1.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptAssertMode {
    Deassert,
    Assert,
}

impl From<InterruptAssertMode> for bool {
    fn from(value: InterruptAssertMode) -> Self {
        match value {
            InterruptAssertMode::Deassert => false,
            InterruptAssertMode::Assert => true,
        }
    }
}

/// Indicates whether a shorthand notation is used to specify the destination of the interrupt and,
/// if so, which shorthand is used. Destination shorthands are used in place of the destination
/// field, and can be sent by software using a single write to the low bits interrupt command register.
pub enum InterruptDestination {
    Processor {
        id: u32,
    },

    /// The issuing APIC is the one and only destination of the inter-process interrupt. This destination
    /// shorthand allows software to interrupt the processor on which it is executing. An APIC
    /// implementation is free to deliver the self-interrupt message internally or to issue the message to
    /// the bus and “snoop” it as with any other inter-process interrupt message.
    OnlySelf,

    /// The inter-process interrupt is sent to all processors in the system including the processor sending
    /// it. The APIC will broadcast an inter-process interrupt message with the destination field set to 0xF
    /// for Pentium and P6 family processors, and to 0xFF for Pentium 4 and Intel Xeon processors.
    AllIncludingSelf,

    /// The inter-process interrupt is sent to all processors in a system with the exception of the processor
    /// sending it. The APIC broadcasts a message with the physical destination mode and destination field set
    /// to 0xF for Pentium and P6 family processors, and to 0xFF for Pentium 4 and Intel Xeon processors.
    /// Support for this destination shorthand in conjunction with the lowest-priority delivery mode is model
    /// specific. For Pentium 4 and Intel Xeon processors, when this shorthand is used together with lowest
    /// priority delivery mode, the inter-process interrupt may be redirected back to the issuing processor.
    AllExclusingSelf,
}

/// Allows software running on the processor to specify and send inter-processor
/// interrupts to other processors in the system.
#[derive(Debug, Clone, Copy)]
pub struct InterruptCommand {
    high: u32,
    low: u32,
}

impl InterruptCommand {
    #[allow(clippy::needless_pass_by_value)]
    pub fn new(
        vector: Option<NonZeroU8>,
        destination: InterruptDestination,
        delivery_mode: InterruptDeliveryMode,
        destination_mode: InterruptDestinationMode,
        trigger_mode: InterruptTriggerMode,
        assert_mode: InterruptAssertMode,
    ) -> Self {
        assert!(
            assert_mode != InterruptAssertMode::Deassert
                || delivery_mode == InterruptDeliveryMode::Init,
            "bit 14 (de-assert) can only be set with INIT delivery mode"
        );
        assert!(
            assert_mode != InterruptAssertMode::Deassert
                || trigger_mode == InterruptTriggerMode::Level,
            "bit 15 (level trigger) must be set with INIT de-assert"
        );
        assert!(
            vector.is_none()
                || !matches!(
                    delivery_mode,
                    InterruptDeliveryMode::SystemManagement | InterruptDeliveryMode::Init
                ),
            "vector should not be specified with SMI or INIT interrupts"
        );

        let mut high = 0u32;
        let mut low = 0u32;

        if let Some(vector) = vector {
            low.set_bits(..8, u32::from(vector.get()));
        }

        low.set_bits(8..11, u32::from(delivery_mode));
        low.set_bit(11, bool::from(destination_mode));
        low.set_bit(14, bool::from(assert_mode));
        low.set_bit(15, bool::from(trigger_mode));

        match destination {
            InterruptDestination::Processor { id } => {
                assert!(
                    assert_mode != InterruptAssertMode::Deassert,
                    "\"all including self\" interrupt destination should be specified with INIT de-assert"
                );

                high = id;
            }

            InterruptDestination::OnlySelf => {
                assert!(
                    assert_mode != InterruptAssertMode::Deassert,
                    "\"all including self\" interrupt destination should be specified with INIT de-assert"
                );

                low.set_bits(18..20, 0b01);
            }

            InterruptDestination::AllIncludingSelf => {
                low.set_bits(18..20, 0b10);
            }

            InterruptDestination::AllExclusingSelf => {
                assert!(
                    assert_mode != InterruptAssertMode::Deassert,
                    "\"all including self\" interrupt destination should be specified with INIT de-assert"
                );

                low.set_bits(18..20, 0b11);
            }
        }

        Self { high, low }
    }

    pub fn new_init(apic_id: u32) -> Self {
        Self::new(
            None,
            InterruptDestination::Processor { id: apic_id },
            InterruptDeliveryMode::Init,
            InterruptDestinationMode::Physical,
            InterruptTriggerMode::Level,
            InterruptAssertMode::Assert,
        )
    }

    pub fn new_sipi(vector: u8, apic_id: u32) -> Self {
        Self::new(
            NonZeroU8::new(vector),
            InterruptDestination::Processor { id: apic_id },
            InterruptDeliveryMode::StartUp,
            InterruptDestinationMode::Physical,
            InterruptTriggerMode::Edge,
            InterruptAssertMode::Assert,
        )
    }

    pub(crate) fn high(self) -> u32 {
        self.high
    }

    pub(crate) fn low(self) -> u32 {
        self.low
    }
}
