use crate::acpi::Signature;
use core::{intrinsics::unaligned_volatile_load, num::NonZero, ptr::NonNull};

pub struct RootSystemDescriptorPointer(NonNull<u8>);

impl RootSystemDescriptorPointer {
    pub unsafe fn from_address(address: usize) -> Self {
        let address = NonZero::<usize>::try_from(address).unwrap();
        Self(NonNull::<u8>::with_exposed_provenance(address))
    }

    /// # Safety
    ///
    /// - `offset` must be a byte-based offset (from the start of the structure
    ///   in memory) to a structure of `T` in memory.
    pub unsafe fn get_field_as<T>(&self, offset: usize) -> T {
        // Safety: Caller is required to maintain safety invariants.
        unsafe { unaligned_volatile_load(self.0.byte_add(offset).cast::<T>().as_ptr()) }
    }

    pub fn get_revision(&self) -> u8 {
        // Safety: ACPI revision is the 16th byte of the root system descriptor pointer.
        unsafe { self.get_field_as::<u8>(15) }
    }

    fn is_v2(&self) -> bool {
        match self.get_revision() {
            2 => true,
            0 => false,
            revision => {
                warn!("ACPI revision invalid (will assume v2+): {revision}");

                true
            }
        }
    }

    pub fn is_checksum_valid(&self) -> bool {
        let length = if self.is_v2() {
            // Safety: If the ACPI version is ≥2.0 then the length is encoded in a 32-bit
            // value at a byte offset of 20.
            usize::try_from(unsafe { self.get_field_as::<u32>(20) }).unwrap()
        } else {
            20
        };

        // Safety: The previously calcualted `length` value is required to be the exact
        // length of the array.
        let bytes = unsafe { core::slice::from_raw_parts(self.0.as_ptr(), length) };
        let checksum = bytes
            .iter()
            .fold(0u8, |accumulator, byte| accumulator.wrapping_add(*byte));

        checksum == 0
    }

    pub fn get_signature(&self) -> Signature<8> {
        // Safety:
        // - `signature` is an 8-byte field at the start of pointer.
        // - `Signature` is `#[repr(transparent)]` over an array of `N` bytes.
        unsafe { self.get_field_as::<Signature<8>>(0) }
    }

    pub fn get_oem_id(&self) -> Signature<6> {
        // Safety:
        // - `signature` is an 8-byte field at the start of pointer.
        // - `Signature` is `#[repr(transparent)]` over an array of `N` bytes.
        unsafe { self.get_field_as::<Signature<6>>(9) }
    }

    pub fn get_rsdt_address(&self) -> Option<NonNull<u8>> {
        let address = {
            if self.is_v2() {
                // Safety:
                // When the ACPI revision is ≥2.0, the address field is a 64-bit zero-based
                // memory offset at byte 24 of the root system descriptor pointer structure.
                let address = unsafe { self.get_field_as::<u64>(24) };
                usize::try_from(address).unwrap()
            } else {
                // Safety:
                // When the ACPI revision is <2.0, the address field is a 32-bit zero-based
                // memory offset at byte 16 of the root system descriptor pointer structure.
                let address = unsafe { self.get_field_as::<u32>(16) };
                usize::try_from(address).unwrap()
            }
        };

        NonZero::<usize>::try_from(address)
            .map(NonNull::with_exposed_provenance)
            .ok()
    }
}

impl core::fmt::Debug for RootSystemDescriptorPointer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Root System Descriptor Pointer")
            .field("Revision", &self.get_revision())
            .field("Signature", &self.get_signature())
            .field("OEM ID", &self.get_oem_id())
            .field("RSDT Address", &self.get_rsdt_address())
            .finish()
    }
}
