use crate::{
    arch::x86_64::registers::model_specific::IA32_APIC_BASE, interrupts::Vector,
    mem::HigherHalfDirectMap, util::sync::Lazy,
};
use bit_field::BitField;
use core::{fmt, num::NonZero, ptr::NonNull};
use libsys::address::{Address, Frame};
use num_enum::{IntoPrimitive, TryFromPrimitive};
use safe_mmio::{
    UniqueMmioPointer,
    fields::{ReadPure, WriteOnly},
};

pub mod interrupt_command;
pub mod local_vector;

pub const US_PER_SEC: u64 = 1_000_000;
pub const US_WAIT: u64 = 10000;
pub const US_FREQ_FACTOR: u64 = US_PER_SEC / US_WAIT;

#[repr(u8)]
#[derive(Debug, IntoPrimitive, Clone, Copy)]
#[allow(non_camel_case_types, clippy::upper_case_acronyms)]
#[rustfmt::skip]
pub enum Register {
    ID                          = 0x02,
    VERSION                     = 0x03,
    TASK_PRIORITY               = 0x08,
    PROCESSOR_PRIORITY          = 0x0A,
    END_OF_INTERRUPT            = 0x0B,
    LOCAL_DESTINATION           = 0x0D,
    SPURIOUS_VECTOR             = 0x0F,
    ERROR_STATUS                = 0x28,
    LVT_CMCI                    = 0x2F,
    LVT_TIMER                   = 0x32,
    LVT_THERMAL_MONITOR         = 0x33,
    LVT_PERFORMANCE_COUNTER     = 0x34,
    LVT_LINT0                   = 0x35,
    LVT_LINT1                   = 0x36,
    LVT_ERROR                   = 0x37,
    TIMER_INITIAL_COUNT         = 0x38,
    TIMER_CURRENT_COUNT         = 0x39,
    TIMER_DIVIDE_CONFIGURATION  = 0x3E,
}

impl Register {
    pub const fn is_readable(self) -> bool {
        match self {
            Register::ID
            | Register::VERSION
            | Register::PROCESSOR_PRIORITY
            | Register::TASK_PRIORITY
            | Register::LOCAL_DESTINATION
            | Register::SPURIOUS_VECTOR
            | Register::ERROR_STATUS
            | Register::TIMER_INITIAL_COUNT
            | Register::TIMER_CURRENT_COUNT
            | Register::TIMER_DIVIDE_CONFIGURATION
            | Register::LVT_CMCI
            | Register::LVT_TIMER
            | Register::LVT_THERMAL_MONITOR
            | Register::LVT_PERFORMANCE_COUNTER
            | Register::LVT_LINT0
            | Register::LVT_LINT1
            | Register::LVT_ERROR => true,

            Register::END_OF_INTERRUPT => false,
        }
    }

    pub const fn is_writable(self) -> bool {
        match self {
            Register::TASK_PRIORITY
            | Register::END_OF_INTERRUPT
            | Register::LOCAL_DESTINATION
            | Register::SPURIOUS_VECTOR
            | Register::ERROR_STATUS
            | Register::TIMER_INITIAL_COUNT
            | Register::TIMER_DIVIDE_CONFIGURATION
            | Register::LVT_CMCI
            | Register::LVT_TIMER
            | Register::LVT_THERMAL_MONITOR
            | Register::LVT_PERFORMANCE_COUNTER
            | Register::LVT_LINT0
            | Register::LVT_LINT1
            | Register::LVT_ERROR => true,

            Register::ID
            | Register::VERSION
            | Register::PROCESSOR_PRIORITY
            | Register::TIMER_CURRENT_COUNT => false,
        }
    }

    pub fn as_xapic_address(self, base_address: Address<Frame>) -> NonZero<usize> {
        let offset = usize::from(u8::from(self)) << 4;
        let address = base_address.get().get() + offset;

        HigherHalfDirectMap::offset(address)
    }

    pub fn as_x2apic_address(self) -> u32 {
        u32::from(u8::from(self)) | 0x800
    }
}

bitflags! {
    #[repr(transparent)]
    #[derive(Debug, Clone, Copy)]
    pub struct ErrorStatus: u32 {
        const SEND_CHECKSUM_ERROR       = 1 << 0;
        const RECEIVE_CHECKSUM_ERROR    = 1 << 1;
        const SEND_ACCEPT_ERROR         = 1 << 2;
        const RECEIVE_ACCEPT_ERROR      = 1 << 3;
        const REDIRECTABLE_IPI          = 1 << 4;
        const SENT_ILLEGAL_VECTOR       = 1 << 5;
        const RECEIVED_ILLEGAL_VECTOR   = 1 << 6;
        const ILLEGAL_REGISTER_ADDRESS  = 1 << 7;
    }
}

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterruptDeliveryMode {
    /// Delivers the interrupt specified in the vector field.
    Fixed,

    /// Note: Only supported for inter-process interrupts. Not supported on x2
    /// APIC.
    ///
    /// Same as fixed mode, except that the interrupt is delivered to the
    /// processor executing at the lowest priority among the set of
    /// processors specified in the destination field. The ability for a
    /// processor to send a lowest priority inter-process interrupt is model
    /// specific and should be avoided by BIOS and operating system
    /// software.
    LowPriority,

    /// Delivers a system management interrupt to the processor core through the
    /// processor’s local system management interrupt signal path. When using
    /// this delivery mode, the vector field should be clear for future
    /// compatibility.
    SystemManagement,

    /// Delivers non-maskable interrupt to the processor. The vector information
    /// is ignored.
    NonMaskable,

    /// Note: Not supported for the LVT CMCI register, the LVT thermal monitor
    /// register, or       the LVT performance counter register.
    ///
    /// Delivers an INIT request to the processor core, which causes the
    /// processor to perform an INIT. When using this delivery mode, the
    /// vector field should be clear for future compatibility.
    ///
    /// **When used by inter-process interrupt with level de-assert**:
    /// (Not supported in the Pentium 4 and Intel Xeon processors.) Sends a
    /// synchronization message to all the local APICs in the system to set
    /// their arbitration IDs (stored in their arbitration ID registers) to
    /// the values of their APIC IDs. For this delivery mode, the level flag
    /// must be set to 0 and trigger mode flag to 1. This inter-process
    /// interrupt is sent to all processors, regardless of the value in the
    /// destination field or the destination shorthand field; however,
    /// software should specify the “all including self” shorthand.
    Init,

    /// Note: Only supported for inter-process interrupts.
    ///
    /// Sends a special “start-up” inter-process interrupt (called a SIPI) to
    /// the target processor or processors. The vector typically points to a
    /// start-up routine that is part of the BIOS boot-strap code.
    /// Inter-process interrupts sent with this delivery mode are not
    /// automatically retried if the source APIC is unable to deliver it. It
    /// is up to the software to determine if the SIPI was not successfully
    /// delivered and to reissue the SIPI if necessary.
    StartUp,

    /// Note: Not supported for inter-process interrupts. Not supported for the
    /// LVT CMCI       register, the LVT thermal monitor register, or the
    /// LVT performance counter       register.
    ///
    /// Causes the processor to respond to the interrupt as if the interrupt
    /// originated in an externally connected (8259A-compatible) interrupt
    /// controller. A special INTA bus cycle corresponding to this mode is
    /// routed to the external controller. The external controller is
    /// expected to supply the vector information. The APIC architecture
    /// supports only one external interrupt source in a system, usually
    /// contained in the compatibility bridge. Only one processor in the
    /// system should have an LVT entry configured to use this delivery
    /// mode.
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

#[repr(u32)]
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

#[derive(Debug, Clone, Copy)]
#[allow(non_camel_case_types)]
enum Mode {
    xApic(Address<Frame>),
    x2Apic,
}

pub struct LocalApic(Mode);

static LOCAL_APIC: Lazy<LocalApic> = Lazy::new(|| {
    assert!(
        IA32_APIC_BASE::get_hw_enabled(),
        "local APIC is hardware disabled"
    );

    let mode = {
        if IA32_APIC_BASE::get_is_x2apic_mode() {
            Mode::x2Apic
        } else {
            Mode::xApic(IA32_APIC_BASE::get_base_address())
        }
    };

    LocalApic(mode)
});

impl LocalApic {
    fn get_mode() -> Mode {
        LOCAL_APIC.0
    }

    fn read_register(register: Register) -> u32 {
        assert!(register.is_readable());

        match Self::get_mode() {
            Mode::xApic(base_address) => {
                let register_address = register.as_xapic_address(base_address);
                let register_ptr =
                    NonNull::<ReadPure<u32>>::with_exposed_provenance(register_address);

                // Safety:
                // - Constructor is required to ensure `base_address` is correct.
                // - Register is checked to be readable.
                // - All APIC registers are 32 bits wide.
                unsafe { UniqueMmioPointer::new(register_ptr) }.read()
            }

            Mode::x2Apic => {
                let register_address = register.as_x2apic_address();

                let value: u32;

                // Safety: Reading from a model-specific register cannot create undefined
                //         behaviour.
                unsafe {
                    core::arch::asm!(
                        "rdmsr",
                        in("ecx") register_address,
                        out("eax") value,
                        out("edx") _,
                        options(nostack, nomem, preserves_flags)
                    );
                }

                value
            }
        }
    }

    /// Writes `value` to `register`.
    ///
    /// # Safety
    ///
    /// - `value` must not contain any invalid values.
    /// - `value` must not contain any reserved bits.
    unsafe fn write_register(register: Register, value: u32) {
        assert!(register.is_writable());

        match Self::get_mode() {
            Mode::xApic(base_address) => {
                let register_address = register.as_xapic_address(base_address);
                let register_ptr =
                    NonNull::<WriteOnly<u32>>::with_exposed_provenance(register_address);

                // Safety:
                // - Constructor is required to ensure `base_address` is correct.
                // - Register is checked to writable.
                // - All APIC registers are 32 bits wide.
                unsafe { UniqueMmioPointer::new(register_ptr) }.write(value);
            }

            Mode::x2Apic => {
                let register_address = register.as_x2apic_address();

                // Safety: Writing to x2 APIC model-specific registers cannot create undefined
                //         behaviour.
                unsafe {
                    core::arch::asm!(
                        "wrmsr",
                        in("ecx") register_address,
                        in("eax") value,
                        in("edx") 0,
                        options(nostack, nomem, preserves_flags)
                    );
                }
            }
        }
    }

    /// The initial ID of the local APIC device.
    pub fn get_id() -> u32 {
        let value = Self::read_register(Register::ID);
        match Self::get_mode() {
            Mode::xApic(_) => value.get_bits(24..32),
            Mode::x2Apic => value,
        }
    }

    /// Version of the APIC device.
    ///
    /// Possible values:
    /// - 0x0_: 82489DX discrete APIC
    /// - 0x10 to 0x15: Integrated APIC
    pub fn version() -> u8 {
        let bits = Self::read_register(Register::VERSION).get_bits(..8);
        u8::try_from(bits).unwrap()
    }

    /// Indicates whether software can inhibit the broadcast of an end of
    /// interrupt message by setting bit 12 of the spurious interrupt vector
    /// register.
    pub fn can_suppress_eoi_broadcast() -> bool {
        Self::read_register(Register::VERSION).get_bit(24)
    }

    /// The number of local vector table entries, less 1.
    ///
    /// Possible values:
    /// - For processors based on the Nehalem microarchitecture (which has 7 LVT
    ///   entries) and onward: 6
    /// - For the Pentium 4 and Intel Xeon processors (which have 6 LVT
    ///   entries): 5
    /// - For the P6 family processors (which have 5 LVT entries): 4
    /// - For the Pentium processor (which has 4 LVT entries): 3
    pub fn max_lvt_entry() -> u8 {
        let bits = Self::read_register(Register::VERSION).get_bits(16..24);
        u8::try_from(bits).unwrap()
    }

    /// Determines the vector number to be delivered to the processor when the
    /// local APIC generates a spurious vector.
    ///
    /// - **For Pentium 4 and Intel Xeon processors**: Bits 0..=7 of the this
    ///   field are programmable by software.
    /// - **For P6 family and Pentium processors**: Bits 4..=7 of the this field
    ///   are programmable by software, and bits 0..=3 are hardwired to logical
    ///   ones.
    ///
    /// # Notes
    ///
    /// A special situation may occur when a processor raises its task priority
    /// to be greater than or equal to the level of the interrupt for which
    /// the processor INTR signal is currently being asserted. If at the
    /// time the INTA cycle is issued, the interrupt that
    /// was to be dispensed has become masked (programmed by software), the
    /// local APIC will deliver a spurious-interrupt vector. Dispensing the
    /// spurious-interrupt vector does not affect the interrupt service
    /// register, so the handler for this vector should return without an
    /// end-of-interrupt call.
    pub fn get_spurious_vector() -> u8 {
        let bits = Self::read_register(Register::SPURIOUS_VECTOR).get_bits(..8);
        u8::try_from(bits).unwrap()
    }

    /// Sets the vector number to be delivered to the processor when the local
    /// APIC generates a spurious vector.
    ///
    /// - **For Pentium 4 and Intel Xeon processors**: Bits 0..=7 of the this
    ///   field are programmable by software.
    /// - **For P6 family and Pentium processors**: Bits 4..=7 of the this field
    ///   are programmable by software, and bits 0..=3 are hardwired to logical
    ///   ones.
    ///
    /// # Notes
    ///
    /// A special situation may occur when a processor raises its task priority
    /// to be greater than or equal to the level of the interrupt for which
    /// the processor INTR signal is currently being asserted. If at the
    /// time the INTA cycle is issued, the interrupt that
    /// was to be dispensed has become masked (programmed by software), the
    /// local APIC will deliver a spurious-interrupt vector. Dispensing the
    /// spurious-interrupt vector does not affect the interrupt service
    /// register, so the handler for this vector should return without an
    /// end-of-interrupt call.
    pub fn set_spurious_vector(vector: Vector) {
        let vector = u8::from(vector);

        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(
                Register::SPURIOUS_VECTOR,
                *Self::read_register(Register::SPURIOUS_VECTOR).set_bits(..8, u32::from(vector)),
            );
        }
    }

    /// Whether the local APIC is enabled (`1`/`true`) or disabled
    /// (`0`/`false`).
    pub fn get_enabled() -> bool {
        Self::read_register(Register::SPURIOUS_VECTOR).get_bit(8)
    }

    /// Enables (`1`/`true`) or disables (`0`/`false`) the local APIC.
    pub fn set_enabled(value: bool) {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(
                Register::SPURIOUS_VECTOR,
                *Self::read_register(Register::SPURIOUS_VECTOR).set_bit(8, value),
            );
        }
    }

    /// Determines whether an end-of-interrupt for a level-triggered interrupt
    /// causes end-of-interrupt messages to be broadcast to the I/O APICs
    /// (`0`/`false`) or not (`1`/`true`). The default value for this bit is
    /// `0`/`false`, indicating that end-of-interrupt broadcasts are
    /// performed. This bit is reserved to `0`/`false` if the processor does
    /// not support end-of-interrupt broadcast suppression.
    pub fn get_eoi_broadcast_suppression() -> bool {
        Self::read_register(Register::SPURIOUS_VECTOR).get_bit(12)
    }

    /// Sets whether an end-of-interrupt for a level-triggered interrupt causes
    /// end-of-interrupt messages to be broadcast to the I/O APICs (`0`/`false`)
    /// or not (`1`/`true`). The default value for this bit is `0`/`false`,
    /// indicating that end-of-interrupt broadcasts are performed. This bit
    /// is reserved to `0`/`false` if the processor does not support
    /// end-of-interrupt broadcast suppression.
    pub fn set_eoi_broadcast_suppression(value: bool) {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(
                Register::SPURIOUS_VECTOR,
                *Self::read_register(Register::SPURIOUS_VECTOR).set_bit(12, value),
            );
        }
    }

    pub fn get_error_status() -> ErrorStatus {
        let bits = Self::read_register(Register::ERROR_STATUS);
        ErrorStatus::from_bits_truncate(bits)
    }

    pub fn clear_error_status() {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(Register::ERROR_STATUS, 0x0);
        }
    }

    pub fn get_timer_initial_count() -> u32 {
        Self::read_register(Register::TIMER_INITIAL_COUNT)
    }

    pub fn set_timer_initial_count(value: u32) {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(Register::TIMER_INITIAL_COUNT, value);
        }
    }

    pub fn get_timer_current_count() -> u32 {
        Self::read_register(Register::TIMER_CURRENT_COUNT)
    }

    pub fn get_timer_divide_configuration() -> TimerDivideConfiguration {
        let bits = Self::read_register(Register::TIMER_DIVIDE_CONFIGURATION);
        TimerDivideConfiguration::try_from(bits).unwrap()
    }

    pub fn set_timer_divide_configuration(value: TimerDivideConfiguration) {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(Register::TIMER_DIVIDE_CONFIGURATION, u32::from(value));
        }
    }

    pub fn send_interrupt_command(interrupt_command: interrupt_command::InterruptCommand) {
        let high_bits = interrupt_command.high_bits();
        let low_bits = interrupt_command.low_bits();

        match Self::get_mode() {
            Mode::xApic(base_address) => {
                const ICR_LOW: usize = 0x300;
                const ICR_HIGH: usize = 0x310;

                let register_low_address =
                    NonZero::<usize>::new(base_address.get().get() + ICR_LOW).unwrap();
                let register_low_ptr =
                    NonNull::<WriteOnly<u32>>::with_exposed_provenance(register_low_address);

                let register_high_address =
                    NonZero::<usize>::new(base_address.get().get() + ICR_HIGH).unwrap();
                let register_high_ptr =
                    NonNull::<WriteOnly<u32>>::with_exposed_provenance(register_high_address);

                // Safety:
                // - Constructor is required to ensure `base_address` is correct.
                // - ICR registers are writable.
                // - ICR registeers are 32 bits wide.
                unsafe {
                    UniqueMmioPointer::new(register_low_ptr).write(low_bits);
                    UniqueMmioPointer::new(register_high_ptr).write(high_bits);
                }
            }

            Mode::x2Apic => {
                const ICR_MSR: u32 = 0x830;

                assert!(
                    low_bits.get_bits(8..11) != 0b001,
                    "x2 APIC does not support low priority delivery mode"
                );

                // Safety:
                // - Caller is required to maintain side-effect safety.
                // - Model-specific register address is correct.
                // - Values are split correctly as low/high dwords.
                unsafe {
                    core::arch::asm!(
                        "wrmsr",
                        in("ecx") ICR_MSR,
                        in("eax") low_bits,
                        in("edx") high_bits,
                        options(nostack, nomem, preserves_flags)
                    );
                }
            }
        }
    }

    /// # Safety
    ///
    /// - Calling context must be the end of an interrupt service routine or
    ///   recoverable processor exception.
    pub unsafe fn end_of_interrupt() {
        // Safety: Value is valid and no reserved bits are set.
        unsafe {
            Self::write_register(Register::END_OF_INTERRUPT, 0x0);
        }
    }

    pub fn reset() {
        trace!("Disabling local APIC for reset sequence...");
        Self::set_enabled(false);

        trace!("Configuring the spurious interrupt...");
        Self::set_spurious_vector(Vector::Spurious);

        // TODO Set up the IO APIC so we can correctly configure these.
        trace!("Configuring the external 0 interrupt...");
        Self::lvt_lint0()
            .set_vector(Vector::External)
            .set_masked(false);
        trace!("Configuring the external 1 interrupt...");
        Self::lvt_lint1()
            .set_vector(Vector::External)
            .set_masked(false);

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

        Self::set_enabled(true);

        debug!("Reset complete.");
    }
}

impl fmt::Debug for LocalApic {
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
