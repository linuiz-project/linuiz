use crate::{acpi::SystemDescriptorTable, util::AsciiStr};
use core::ptr::NonNull;

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct EmulatedDeviceFlags: u32 {
        /// Indicates whether the RTC has been enhanced not to require
        /// acknowledgment after it asserts an interrupt. With this bit set, an
        /// interrupt handler can bypass reading the RTC register C to unlatch
        /// the pending interrupt.
        const RTC_GOOD = 1 << 0;

        /// Indicates whether the ACPI PM timer has been enhanced not to require
        /// multiple reads. With this bit set, only one read of the ACPI PM
        /// timer is necessary to obtain a reliable value.
        const PM_TIMER_GOOD = 1 << 1;
    }
}

/// The WAET (Windows ACPI Emulated Devices Table) is a table in ACPI running in
/// guest partitions in a virtual machine environment.
pub struct Waet(NonNull<u8>);

unsafe impl SystemDescriptorTable for Waet {
    const SIGNATURE: crate::util::AsciiStr<4> = AsciiStr::new(*b"WAET").unwrap();

    fn base_ptr(&self) -> NonNull<u8> {
        self.0
    }
}

impl Waet {
    pub const unsafe fn new(base_ptr: NonNull<u8>) -> Self {
        Self(base_ptr)
    }

    pub fn emulated_device_flags(&self) -> EmulatedDeviceFlags {
        let value = unsafe { self.read_offset_as::<u32>(36) };
        EmulatedDeviceFlags::from_bits_truncate(value)
    }
}

impl core::fmt::Debug for Waet {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("Windows Emulated Device Table");
        self.write_header_debug_fields(&mut d);
        d.field("Emulated Device Flags", &self.emulated_device_flags())
            .finish()
    }
}
