use crate::{
    acpi::SystemDescriptorTable,
    util::{AsciiStr, Version},
};
use bit_field::BitField;
use core::{num::NonZero, ptr::NonNull};
use ioports::ReadOnlyPort;
use num_enum::{FromPrimitive, IntoPrimitive};

mod pm_timer;
pub use pm_timer::PmTimer;

#[repr(u8)]
#[derive(Debug, FromPrimitive, IntoPrimitive, Clone, Copy, PartialEq, Eq)]
pub enum PowerManagementProfile {
    Unspecified = 0,
    Desktop = 1,
    Mobile = 2,
    Workstation = 3,
    EnterpriseServer = 4,
    SohoServer = 5,
    AppliancePc = 6,
    PerformanceServer = 7,
    Tablet = 8,

    #[default]
    Unknown = u8::MAX,
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
bitflags! {
    /// This set of flags is used by an OS to guide the assumptions it can make
    /// in initializing hardware on IA-PC platforms. These flags are used by an
    /// OS at boot time (before the OS is capable of providing an operating
    /// environment suitable for parsing the ACPI namespace) to determine the
    /// code paths to take during boot. In IA-PC platforms with reduced legacy
    /// hardware, the OS can skip code paths for legacy devices if none are
    /// present. For example, if there are no ISA devices, an OS could skip code
    /// that assumes the presence of these devices and their associated resources.
    ///
    /// These flags are used independently of the ACPI namespace.
    ///
    /// The presence of other devices must be described in the ACPI namespace as
    /// specified in Section 6 of the ACPI specification.
    ///
    /// These flags pertain only to IA-PC platforms. On other system
    /// architectures, the entire field should be set to 0.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BootArchitectureFlags: u16 {
        /// If set, indicates that the motherboard supports user-visible devices
        /// on the LPC or ISA bus. User-visible devices are devices that have
        /// end-user accessible connectors (for example, LPT port), or devices
        /// for which the OS must load a device driver so that an end-user
        /// application can use a device.
        ///
        /// If clear, the OS may assume there are no such devices and that all
        /// devices in the system can be detected exclusively via industry
        /// standard device enumeration mechanisms (including the ACPI namespace).
        const LEGACY_DEVICES = 1 << 0;

        /// If set, indicates that the motherboard contains support for a port
        /// 60 and 64 based keyboard controller, usually implemented as an 8042
        /// or equivalent micro-controller.
        const CONTROLLER_8042 = 1 << 1;

        /// If set, indicates to OSPM that it must not blindly probe the VGA
        /// hardware (that responds to MMIO addresses `0xA0000-0xBFFFF` and IO
        /// ports `0x3B0-0x3BB` and `0x3C0-0x3DF`) that may cause machine check
        /// on this system.
        ///
        /// If clear, indicates to OSPM that it is safe to probe the VGA hardware.
        const VGA_NOT_PRESENT = 1 << 2;

        /// If set, indicates to OSPM that it must not enable Message Signaled
        /// Interrupts (MSI) on this platform.
        const MSI_NOT_SUPPORTED = 1 << 3;

        /// If set, indicates to OSPM that it must not enable OSPM ASPM control
        /// on this platform.
        const PCIE_ASPM_CONTROLS = 1 << 4;

        /// If set, indicates that the CMOS RTC is either not implemented, or
        /// does not exist at the legacy addresses. OSPM uses the "Control Method
        /// Time and Alarm Namespace" device instead.
        const CMOS_RTC_NOT_PRESENT = 1 << 5;
    }


}

#[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
bitflags! {
    /// These flags are used by an OS at boot time (before the OS is capable of
    /// providing an operating environment suitable for parsing the ACPI
    /// namespace) to determine the code paths to take during boot. For the PSCI
    /// flags, specifically, the flags describe if the platform is compliant
    /// with the PSCI specification. A link to the PSCI specification can be
    /// found at “Links to ACPI-Related Documents” at http://uefi.org/acpi.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct BootArchitectureFlags: u16 {
        /// If set, PSCI is implemented.
        const PSCI_COMPLIANT = 1 << 0;

        /// If set, HVC must be used as the PSCI conduit (instead of SMC).
        const PSCI_USE_HVC = 1 << 1;
    }
}

/// The Fixed ACPI Description Table defines various fixed hardware ACPI
/// information vital to an ACPI-compatible OS.
///
/// The FADT also has a pointer to the DSDT that contains the Differentiated
/// Definition Block, which in turn provides variable information to an
/// ACPI-compatible OS concerning the base system design.
pub struct Fadt(NonNull<u8>);

// Safety: `Self::new` requires `self.0` be a valid base pointer.
unsafe impl SystemDescriptorTable for Fadt {
    const SIGNATURE: AsciiStr<4> = AsciiStr::new(*b"FACP").unwrap();

    fn base_ptr(&self) -> NonNull<u8> {
        self.0
    }
}

impl Fadt {
    pub const unsafe fn new(base_ptr: NonNull<u8>) -> Self {
        Self(base_ptr)
    }

    pub fn version(&self) -> Version {
        // Safety: `major` is 1 bytes @ offset 8.
        let major = unsafe { self.read_offset_as::<u8>(8) };
        let (minor, patch) = {
            // Safety: `minor` and `patch` share 1 byte at offset 131.
            let value = unsafe { self.read_offset_as::<u8>(131) };
            (value.get_bits(..4), value.get_bits(4..))
        };

        Version::new(major, minor, patch)
    }

    // TODO firmware_ctrl / x_firmware_ctrl: off 32, sz 4
    // TODO dsdt / x_dsdt address: off 40, sz 4

    /// This field is set by the OEM to convey the preferred power management
    /// profile to OSPM. OSPM can use this field to set default power management
    /// policy parameters during OS installation.
    pub fn preferred_power_management_profile(&self) -> PowerManagementProfile {
        // Safety: `preferred_power_management_profile` is 1 bytes @ offset 45.
        let value = unsafe { self.read_offset_as::<u8>(45) };
        PowerManagementProfile::from(value)
    }

    /// System vector the SCI interrupt is wired to in 8259 mode. On systems
    /// that do not contain the 8259, this field contains the Global System
    /// interrupt number of the SCI interrupt. OSPM is required to treat the
    /// ACPI SCI interrupt as a sharable, level, active low interrupt.
    pub fn sci_interrupt_vector(&self) -> u16 {
        // Safety: `sci_interrupt_vector` is 2 bytes @ offset 46.
        unsafe { self.read_offset_as::<u16>(46) }
    }

    /// System port address of the SMI Command Port. During ACPI OS
    /// initialization, OSPM can determine that the ACPI hardware registers are
    /// owned by SMI (by way of the `SCI_EN` bit), in which case the ACPI OS
    /// issues the `ACPI_ENABLE` command to the `SMI_CMD` port. The `SCI_EN` bit
    /// effectively tracks the ownership of the ACPI hardware registers. OSPM
    /// issues commands to the `SMI_CMD` port synchronously from the boot
    /// processor. This field is reserved and must be zero on system that does
    /// not support System Management mode.
    pub fn sci_port_address(&self) -> Option<NonZero<u32>> {
        // Safety: `sci_port_address` is 4 bytes @ offset 48.
        let value = unsafe { self.read_offset_as::<u32>(48) };
        NonZero::<u32>::new(value)
    }

    /// The value to write to `SMI_CMD` to disable SMI ownership of the ACPI
    /// hardware registers. The last action SMI does to relinquish ownership is
    /// to set the `SCI_EN` bit. During the OS initialization process, OSPM will
    /// synchronously wait for the ntransfer of SMI ownership to complete, so
    /// the ACPI system releases SMI ownership as quickly as possible. This
    /// field is reserved and must be zero on systems that do not support Legacy
    /// Mode.
    pub fn sci_acpi_enable_command(&self) -> Option<NonZero<u8>> {
        // Safety: `sci_acpi_enable_command` is 1 bytes @ offset 52.
        let value = unsafe { self.read_offset_as::<u8>(52) };
        NonZero::<u8>::new(value)
    }

    /// The value to write to `SMI_CMD` to re-enable SMI ownership of the ACPI
    /// hardware registers. This can only be done when ownership was originally
    /// acquired from SMI by OSPM using `ACPI_ENABLE`. An OS can hand ownership
    /// back to SMI by relinquishing use to the ACPI hardware registers, masking
    /// off all SCI interrupts, clearing the `SCI_EN` bit and then writing
    /// `ACPI_DISABLE` to the `SMI_CMD` port from the boot processor. This field
    /// is reserved and must be zero on systems that do not support Legacy
    /// Mode.
    pub fn sci_acpi_disable_command(&self) -> Option<NonZero<u8>> {
        // Safety: `sci_acpi_disable_command` is 1 bytes @ offset 53.
        let value = unsafe { self.read_offset_as::<u8>(53) };
        NonZero::<u8>::new(value)
    }

    /// The value to write to `SMI_CMD` to enter the S4BIOS state. The S4BIOS
    /// state provides an alternate way to enter the S4 state where the firmware
    /// saves and restores the memory context. A value of zero in `S4BIOS_F`
    /// indicates `S4BIOS_REQ` is not supported. (See Section 5.2.10 of the ACPI
    /// specification)
    pub fn sci_s4bios_req_command(&self) -> Option<NonZero<u8>> {
        // Safety: `sci_s4bios_req_command` is 1 bytes @ offset 54.
        let value = unsafe { self.read_offset_as::<u8>(54) };
        NonZero::<u8>::new(value)
    }

    /// If non-zero, this field contains the value OSPM writes to the `SMI_CMD`
    /// register to assume processor performance state control responsibility.
    pub fn sci_pstate_control_command(&self) -> Option<NonZero<u8>> {
        // Safety: `sci_pstate_control_command` is 1 bytes @ offset 55.
        let value = unsafe { self.read_offset_as::<u8>(55) };
        NonZero::<u8>::new(value)
    }

    /// If non-zero, this field contains the value OSPM writes to the `SMI_CMD`
    /// register to indicate OS support for the _CST object and C States Changed
    /// notification.
    pub fn sci_cstate_control_command(&self) -> Option<NonZero<u8>> {
        // Safety: `sci_cstate_control_command` is 1 bytes @ offset 96.
        let value = unsafe { self.read_offset_as::<u8>(96) };
        NonZero::<u8>::new(value)
    }

    /// If `WBINVD=0`, the value of this field is the number of flush strides
    /// that need to be read (using cacheable addresses) to completely flush
    /// dirty lines from any processor’s memory caches. Notice that the
    /// value in `FLUSH_STRIDE` is typically the smallest cache line width
    /// on any of the processor’s caches (for more information, see the
    /// `FLUSH_STRIDE` field definition). If the system does not support a
    /// method for flushing the processor’s caches, then `FLUSH_SIZE` and
    /// `WBINVD` are set to zero. Notice that this method of flushing the
    /// processor caches has limitations, and `WBINVD=1` is the preferred
    /// way to flush the processors caches. This value is typically at least
    /// 2 times the cache size. The maximum allowed value for `FLUSH_SIZE`
    /// multiplied by `FLUSH_STRIDE` is 2 MB for a typical maximum supported
    /// cache size of 1 MB. Larger cache sizes are supported using
    /// `WBINVD=1`. This value is ignored if `WBINVD=1`. This field is
    /// maintained for ACPI 1.0 processor compatibility on existing systems.
    /// Processors in new ACPI-compatible systems are required to support
    /// the `WBINVD` function and indicate this to OSPM by setting the
    /// `WBINVD` field = 1.
    pub fn processor_cache_flush_size(&self) -> u16 {
        // Safety: `processor_cache_flush_size` is 2 bytes @ offset 100.
        unsafe { self.read_offset_as::<u16>(100) }
    }

    /// If `WBINVD=0`, the value of this field is the cache line width, in
    /// bytes, of the processor’s memory caches. This value is typically the
    /// smallest cache line width on any of the processor’s caches. For more
    /// information, see the description of the `FLUSH_SIZE` field. This value
    /// is ignored if `WBINVD=1`. This field is maintained for ACPI 1.0
    /// processor compatibility on existing systems. Processors in new
    /// ACPI-compatible systems are required to support the `WBINVD`
    /// function and indicate this to OSPM by setting `WBINVD=1`.
    pub fn processor_cache_flush_stride(&self) -> u16 {
        // Safety: `processor_cache_flush_stride` is 2 bytes @ offset 102.
        unsafe { self.read_offset_as::<u16>(102) }
    }

    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    /// Boot architecture flags.
    pub fn boot_architecture_flags(&self) -> BootArchitectureFlags {
        // Safety: `boot_architecture_flags` is 2 bytes @ offset 109.
        let value = unsafe { self.read_offset_as::<u16>(109) };
        BootArchitectureFlags::from_bits_truncate(value)
    }

    #[cfg(any(target_arch = "arm", target_arch = "aarch64"))]
    /// Boot architecture flags.
    pub fn boot_architecture_flags(&self) -> BootArchitectureFlags {
        let value = unsafe { self.read_offset_as::<u16>(129) };
        BootArchitectureFlags::from_bits_truncate(value)
    }

    /// 64-bit identifier of hypervisor vendor. All bytes in this field are
    /// considered part of the vendor identity. These identifiers are defined
    /// independently by the vendors themselves, usually following the name of
    /// the hypervisor product. Version information should NOT be included in
    /// this field - this shall simply denote the vendor’s name or identifier.
    /// Version information can be communicated through a supplemental
    /// vendor-specific hypervisor API. Firmware implementers would place zero
    /// bytes into this field, denoting that no hypervisor is present in the
    /// actual firmware.
    pub fn hypervisor_vendor(&self) -> Option<AsciiStr<8>> {
        // Safety: `hypervisor_vendor` is 8 bytes @ offset 268.
        let bytes = unsafe { self.read_offset_as::<[u8; 8]>(268) };
        (bytes != [0u8; 8]).then_some(AsciiStr::new_lossy(bytes))
    }

    pub fn pm_timer(&self) -> PmTimer {
        fn read_pm_timer_blk(fadt: &Fadt) -> u32 {
            // Safety: `pm_timer_blk` is 4 bytes @ offset 76.
            unsafe { fadt.read_offset_as::<u32>(76) }
        }

        fn read_x_pm_timer_blk(fadt: &Fadt) -> Option<NonNull<u32>> {
            // Safety: `x_pm_timer_blk` address is 8 bytes @ offset 212.
            let address = unsafe { fadt.read_offset_as::<u64>(212) };
            let address = usize::try_from(address).unwrap();

            NonZero::<usize>::new(address).map(NonNull::<u32>::with_exposed_provenance)
        }

        pub fn is_pm_timer_32_bit(fadt: &Fadt) -> bool {
            // Safety: `pm_timer_len` is 4 bytes @ offset 91.
            match unsafe { fadt.read_offset_as::<u8>(91) } {
                0 => false,
                4 => true,

                pm_timer_len => unreachable!("PM Timer Len: {pm_timer_len}"),
            }
        }

        if self.length() >= 208
            && let Some(timer_ptr) = read_x_pm_timer_blk(self)
        {
            // Safety: `source` is the implemented and correct address.
            unsafe {
                PmTimer::new(
                    pm_timer::Source::MemoryIo(timer_ptr),
                    is_pm_timer_32_bit(self),
                )
            }
        } else {
            let timer_port = read_pm_timer_blk(self);
            let timer_port = u16::try_from(timer_port).unwrap();
            // Safety: Firmware guarantees accuracy of the `PM_TIMER_BLK` address.
            let timer_port = unsafe { ReadOnlyPort::<u32>::new(timer_port) };

            // Safety: `source` is the implemented and correct address.
            unsafe {
                PmTimer::new(
                    pm_timer::Source::PortIo(timer_port),
                    is_pm_timer_32_bit(self),
                )
            }
        }
    }
}

impl core::fmt::Debug for Fadt {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("Fadt");

        d.field("Version", &self.version());

        self.write_header_debug_fields(&mut d);

        d.field(
            "Preferred Power Management Profile",
            &self.preferred_power_management_profile(),
        )
        .field("SCI Interrupt Vector", &self.sci_interrupt_vector())
        .field("SCI Port Address", &self.sci_port_address())
        .field("SCI ACPI Enable Command", &self.sci_acpi_enable_command())
        .field("SCI ACPI Disable Command", &self.sci_acpi_disable_command())
        .field("SCI S4BIOS Request Command", &self.sci_s4bios_req_command())
        .field(
            "SCI P-State Control Command",
            &self.sci_pstate_control_command(),
        )
        .field(
            "SCI C-State Control Command",
            &self.sci_cstate_control_command(),
        )
        .field(
            "Processor Cache Flush Size",
            &self.processor_cache_flush_size(),
        )
        .field(
            "Processor Cache Flush Stride",
            &self.processor_cache_flush_stride(),
        )
        .field("Boot Architecture Flags", &self.boot_architecture_flags())
        .field("Hypervisor Vendor", &self.hypervisor_vendor())
        .finish()
    }
}
