pub mod interrupt_command;
pub mod local_vector;

use crate::interrupts::Vector;
use bit_field::BitField;
use core::fmt;

pub const US_PER_SEC: u64 = 1000000;
pub const US_WAIT: u64 = 10000;
pub const US_FREQ_FACTOR: u64 = US_PER_SEC / US_WAIT;

#[repr(u32)]
#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[allow(non_camel_case_types)]
#[rustfmt::skip]
pub enum Register {
    ID                          = 0x802,
    VERSION                     = 0x803,
    TASK_PRIORITY               = 0x808,
    PROCESSOR_PRIORITY          = 0x80A,
    END_OF_INTERRUPT            = 0x80B,
    LOCAL_DESTINATION           = 0x80D,
    SPURIOUS_VECTOR             = 0x80F,
    ERROR_STATUS                = 0x828,
    LVT_CMCI                    = 0x82F,
    INTERRUPT_COMMAND           = 0x830,
    LVT_TIMER                   = 0x832,
    LVT_THERMAL_MONITOR         = 0x833,
    LVT_PERFORMANCE_COUNTER    = 0x834,
    LVT_LINT0                   = 0x835,
    LVT_LINT1                   = 0x836,
    LVT_ERROR                   = 0x837,
    TIMER_INITIAL_COUNT         = 0x838,
    TIMER_CURRENT_COUNT         = 0x839,
    TIMER_DIVIDE_CONFIGURATION  = 0x83E,
}

/// Reads from the model-specific register at the provided `address`.
#[inline(always)]
fn read_register(register: Register) -> u64 {
    let value_low: u64;
    let value_high: u64;

    // Safety: Reading from a model-specific register cannot create undefined behaviour.
    unsafe {
        core::arch::asm!(
            "rdmsr",
            in("ecx") u32::from(register),
            out("edx") value_high,
            out("eax") value_low,
            options(nostack, nomem, preserves_flags)
        );
    }

    (value_high << 32) | value_low
}

/// Writes `value` to the model-specific register at the provided `address`.
#[inline(always)]
fn write_register(register: Register, value: u64) {
    let value_low = value & 0xFFFF_FFFF;
    let value_high = value >> 32;

    // Safety: Writing to x2 APIC model-specific registers cannot create undefined behaviour.
    unsafe {
        core::arch::asm!(
            "wrmsr",
            in("ecx") u32::from(register),
            in("edx") value_high,
            in("eax") value_low,
            options(nostack, nomem, preserves_flags)
        );
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    pub struct ErrorStatus: u64 {
        const SEND_CHECKSUM_ERROR = 1 << 0;
        const RECEIVE_CHECKSUM_ERROR = 1 << 1;
        const SEND_ACCEPT_ERROR = 1 << 2;
        const RECEIVE_ACCEPT_ERROR = 1 << 3;
        const REDIRECTABLE_IPI = 1 << 4;
        const SENT_ILLEGAL_VECTOR = 1 << 5;
        const RECEIVED_ILLEGAL_VECTOR = 1 << 6;
        const ILLEGAL_REGISTER_ADDRESS = 1 << 7;
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    /// Delivers the interrupt specified in the vector field.
    Fixed,

    /// Note: Only supported for inter-process interrupts. Not supported on x2 APIC.
    ///
    /// Same as fixed mode, except that the interrupt is delivered to the processor
    /// executing at the lowest priority among the set of processors specified in
    /// the destination field. The ability for a processor to send a lowest priority
    /// inter-process interrupt is model specific and should be avoided by BIOS and
    /// operating system software.
    LowPriority,

    /// Delivers a system management interrupt to the processor core through the
    /// processor’s local system management interrupt signal path. When using this
    /// delivery mode, the vector field should be clear for future compatibility.
    SystemManagement,

    /// Delivers non-maskable interrupt to the processor. The vector information is ignored.
    NonMaskable,

    /// Note: Not supported for the LVT CMCI register, the LVT thermal monitor register, or
    ///       the LVT performance counter register.
    ///
    /// Delivers an INIT request to the processor core, which causes the processor to perform
    /// an INIT. When using this delivery mode, the vector field should be clear for future
    /// compatibility.
    ///
    /// **When used by inter-process interrupt with level de-assert**:
    /// (Not supported in the Pentium 4 and Intel Xeon processors.) Sends a synchronization
    /// message to all the local APICs in the system to set their arbitration IDs (stored in
    /// their arbitration ID registers) to the values of their APIC IDs. For this delivery
    /// mode, the level flag must be set to 0 and trigger mode flag to 1. This inter-process
    /// interrupt is sent to all processors, regardless of the value in the destination field
    /// or the destination shorthand field; however, software should specify the “all including
    /// self” shorthand.
    Init,

    /// Note: Only supported for inter-process interrupts.
    ///
    /// Sends a special “start-up” inter-process interrupt (called a SIPI) to the target
    /// processor or processors. The vector typically points to a start-up routine that is
    /// part of the BIOS boot-strap code. Inter-process interrupts sent with this delivery
    /// mode are not automatically retried if the source APIC is unable to deliver it. It
    /// is up to the software to determine if the SIPI was not successfully delivered and
    /// to reissue the SIPI if necessary.
    StartUp,

    /// Note: Not supported for inter-process interrupts. Not supported for the LVT CMCI
    ///       register, the LVT thermal monitor register, or the LVT performance counter
    ///       register.
    ///
    /// Causes the processor to respond to the interrupt as if the interrupt originated in
    /// an externally connected (8259A-compatible) interrupt controller. A special INTA bus
    /// cycle corresponding to this mode is routed to the external controller. The external
    /// controller is expected to supply the vector information. The APIC architecture
    /// supports only one external interrupt source in a system, usually contained in the
    /// compatibility bridge. Only one processor in the system should have an LVT entry
    /// configured to use this delivery mode.
    External,
}

impl From<InterruptDeliveryMode> for u32 {
    fn from(value: InterruptDeliveryMode) -> Self {
        match value {
            InterruptDeliveryMode::Fixed => 0b000,
            InterruptDeliveryMode::LowPriority => 0b001,
            InterruptDeliveryMode::SystemManagement => 0b010,
            InterruptDeliveryMode::NonMaskable => 0b100,
            InterruptDeliveryMode::Init => 0b101,
            InterruptDeliveryMode::StartUp => 0b110,
            InterruptDeliveryMode::External => 0b111,
        }
    }
}

#[repr(u64)]
#[derive(Debug, TryFromPrimitive, IntoPrimitive, Clone, Copy)]
pub enum TimerDivideConfiguration {
    DivideBy1 = 0b1011,
    DivideBy2 = 0b0000,
    DivideBy4 = 0b0001,
    DivideBy8 = 0b0010,
    DivideBy16 = 0b0011,
    DivideBy32 = 0b1000,
    DivideBy64 = 0b1001,
    DivideBy128 = 0b1010,
}

#[allow(non_camel_case_types)]
pub struct x2Apic;

impl x2Apic {
    pub fn reset() {
        debug!("Local APIC:\n{x2Apic:#X?}");

        trace!("Disabling local APIC for reset sequence...");
        Self::set_enabled(false);

        trace!("Configuring the spurious interrupt...");
        Self::set_spurious_vector(Vector::Spurious);

        // TODO Set up the IO APIC so we can correctly configure these.
        // trace!("Configuring the external 0 interrupt...");
        // LocalVector::<LINT0>::set_vector(Vector::External);
        // LocalVector::<LINT0>::set_masked(false);
        // trace!("Configuring the external 1 interrupt...");
        // LocalVector::<LINT1>::set_vector(Vector::External);
        // LocalVector::<LINT1>::set_masked(false);

        trace!("Configuring the error interrupt...");
        Self::lvt_error()
            .set_vector(Vector::Error)
            .set_masked(false);

        trace!("Configuring the timer interrupt (will be masked)...");
        Self::lvt_timer().set_vector(Vector::Timer).set_masked(true);

        if let Some(lvt_performance_counter) = Self::lvt_performance_counter() {
            trace!("Configuring the performance counter interrupt...");
            lvt_performance_counter
                .set_vector(Vector::PerformanceCounter)
                .set_masked(false);
        } else {
            trace!("Performance counter local vector not supported.");
        }

        if let Some(lvt_thermal_monitor) = Self::lvt_thermal_monitor() {
            trace!("Configuring the thermal monitor interrupt...");
            lvt_thermal_monitor
                .set_vector(Vector::ThermalSensor)
                .set_masked(false);
        } else {
            trace!("Thermal monitor local vector not supported.");
        }

        if let Some(lvt_cmci) = Self::lvt_cmci() {
            trace!("Configuring the CMCI interrupt...");
            lvt_cmci.set_vector(Vector::CMCI).set_masked(false);
        } else {
            trace!("CMCI local vector not supported.");
        }

        debug!("Local APIC reset.");
    }

    /// The initial ID of the local APIC device.
    pub fn get_id() -> u32 {
        u32::try_from(read_register(Register::ID)).unwrap()
    }

    /// Version of the APIC device.
    ///
    /// Possible values:
    /// - 0x0_: 82489DX discrete APIC
    /// - 0x10 to 0x15: Integrated APIC
    pub fn version() -> u8 {
        u8::try_from(read_register(Register::VERSION).get_bits(..8)).unwrap()
    }

    /// Indicates whether software can inhibit the broadcast of an end of interrupt
    /// message by setting bit 12 of the spurious interrupt vector register.
    pub fn can_suppress_eoi_broadcast() -> bool {
        read_register(Register::VERSION).get_bit(24)
    }

    /// The number of local vector table entries, less 1.
    ///
    /// Possible values:
    /// - For processors based on the Nehalem microarchitecture (which has 7 LVT entries) and onward: 6
    /// - For the Pentium 4 and Intel Xeon processors (which have 6 LVT entries): 5
    /// - For the P6 family processors (which have 5 LVT entries): 4
    /// - For the Pentium processor (which has 4 LVT entries): 3
    pub fn max_lvt_entry() -> u8 {
        u8::try_from(read_register(Register::VERSION).get_bits(16..24)).unwrap()
    }

    /// Determines the vector number to be delivered to the processor when the local
    /// APIC generates a spurious vector.
    ///
    /// - **For Pentium 4 and Intel Xeon processors**: Bits 0..=7 of the this field are
    ///   programmable by software.
    /// - **For P6 family and Pentium processors**: Bits 4..=7 of the this field are
    ///   programmable by software, and bits 0..=3 are hardwired to logical ones.
    ///
    /// # Notes
    ///
    /// A special situation may occur when a processor raises its task priority to be greater
    /// than or equal to the level of the interrupt for which the processor INTR signal is
    /// currently being asserted. If at the time the INTA cycle is issued, the interrupt that
    /// was to be dispensed has become masked (programmed by software), the local APIC will
    /// deliver a spurious-interrupt vector. Dispensing the spurious-interrupt vector does not
    /// affect the interrupt service register, so the handler for this vector should return
    /// without an end-of-interrupt call.
    pub fn get_spurious_vector() -> u8 {
        let vector = read_register(Register::SPURIOUS_VECTOR).get_bits(..8);

        debug_assert!(vector > 15, "interrupts vectors 0..=15 are reserved");

        u8::try_from(vector).unwrap()
    }

    /// Sets the vector number to be delivered to the processor when the local APIC
    /// generates a spurious vector.
    ///
    /// - **For Pentium 4 and Intel Xeon processors**: Bits 0..=7 of the this field are
    ///   programmable by software.
    /// - **For P6 family and Pentium processors**: Bits 4..=7 of the this field are
    ///   programmable by software, and bits 0..=3 are hardwired to logical ones.
    ///
    /// # Notes
    ///
    /// A special situation may occur when a processor raises its task priority to be greater
    /// than or equal to the level of the interrupt for which the processor INTR signal is
    /// currently being asserted. If at the time the INTA cycle is issued, the interrupt that
    /// was to be dispensed has become masked (programmed by software), the local APIC will
    /// deliver a spurious-interrupt vector. Dispensing the spurious-interrupt vector does not
    /// affect the interrupt service register, so the handler for this vector should return
    /// without an end-of-interrupt call.
    pub fn set_spurious_vector(vector: Vector) {
        let vector = u8::from(vector);

        assert!(vector > 15, "interrupts vectors 0..=15 are reserved");

        write_register(
            Register::SPURIOUS_VECTOR,
            *read_register(Register::SPURIOUS_VECTOR).set_bits(..8, u64::from(vector)),
        );
    }

    /// Whether the local APIC is enabled (`1`/`true`) or disabled (`0`/`false`).
    pub fn get_enabled() -> bool {
        read_register(Register::SPURIOUS_VECTOR).get_bit(8)
    }

    /// Enables (`1`/`true`) or disables (`0`/`false`) the local APIC.
    pub fn set_enabled(value: bool) {
        write_register(
            Register::SPURIOUS_VECTOR,
            *read_register(Register::SPURIOUS_VECTOR).set_bit(8, value),
        );
    }

    /// Determines whether an end-of-interrupt for a level-triggered interrupt causes
    /// end-of-interrupt messages to be broadcast to the I/O APICs (`0`/`false`) or not
    /// (`1`/`true`). The default value for this bit is `0`/`false`, indicating that
    /// end-of-interrupt broadcasts are performed. This bit is reserved to `0`/`false`
    /// if the processor does not support end-of-interrupt broadcast suppression.
    pub fn get_eoi_broadcast_suppression() -> bool {
        read_register(Register::SPURIOUS_VECTOR).get_bit(12)
    }

    /// Sets whether an end-of-interrupt for a level-triggered interrupt causes
    /// end-of-interrupt messages to be broadcast to the I/O APICs (`0`/`false`) or not
    /// (`1`/`true`). The default value for this bit is `0`/`false`, indicating that
    /// end-of-interrupt broadcasts are performed. This bit is reserved to `0`/`false`
    /// if the processor does not support end-of-interrupt broadcast suppression.
    pub fn set_eoi_broadcast_suppression(value: bool) {
        write_register(
            Register::SPURIOUS_VECTOR,
            *read_register(Register::SPURIOUS_VECTOR).set_bit(12, value),
        );
    }

    pub fn get_error_status() -> ErrorStatus {
        ErrorStatus::from_bits_truncate(read_register(Register::ERROR_STATUS))
    }

    fn clear_error_status() {
        write_register(Register::ERROR_STATUS, 0x0);
    }

    pub fn get_timer_initial_count() -> u32 {
        u32::try_from(read_register(Register::TIMER_INITIAL_COUNT)).unwrap()
    }

    pub fn set_timer_initial_count(value: u32) {
        write_register(Register::TIMER_INITIAL_COUNT, u64::from(value));
    }

    pub fn get_timer_current_count() -> u32 {
        u32::try_from(read_register(Register::TIMER_CURRENT_COUNT)).unwrap()
    }

    pub fn get_timer_divide_configuration() -> TimerDivideConfiguration {
        TimerDivideConfiguration::try_from(read_register(Register::TIMER_DIVIDE_CONFIGURATION))
            .unwrap()
    }

    pub fn set_timer_divide_configuration(value: TimerDivideConfiguration) {
        write_register(Register::TIMER_DIVIDE_CONFIGURATION, u64::from(value));
    }

    pub fn send_interrupt_command(interrupt_command: interrupt_command::InterruptCommand) {
        let high = u64::from(interrupt_command.high());
        let low = u64::from(interrupt_command.low());

        assert!(
            low.get_bits(8..11) != 0b001,
            "x2 APIC does not support low priority delivery mode"
        );

        write_register(Register::INTERRUPT_COMMAND, (high << 32) | low);
    }

    pub fn end_of_interrupt() {
        write_register(Register::END_OF_INTERRUPT, 0x0);
    }
}

impl fmt::Debug for x2Apic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Version")
            .field("ID", &Self::get_id())
            .field("Version", &Self::version())
            .field(
                "Can Suppress EOI Broadcast",
                &Self::can_suppress_eoi_broadcast(),
            )
            .field("Maximum LVT Entry", &Self::max_lvt_entry())
            .finish()
    }
}
