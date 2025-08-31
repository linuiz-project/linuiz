use crate::{acpi::SystemDescriptorTable, util::AsciiStr};
use core::{marker::PhantomData, ptr::NonNull};

mod iter;
pub use iter::*;

/// # Safety
///
/// - `Self::ENTRY_SIZE` must be 4 for the RSDT, or 8 for the XSDT.
pub unsafe trait RsdtKind {
    /// Signature of the table ("RSDT"/"XSDT").
    const SIGNATURE: AsciiStr<4>;

    /// Size of the table's entries (4 for RSDT, 8 for XSDT).
    const ENTRY_SIZE: usize;
}

pub struct Standard;
// Safety: `Self::ENTRY_SIZE` is 4 (for RSDT).
unsafe impl RsdtKind for Standard {
    const SIGNATURE: AsciiStr<4> = AsciiStr::new(*b"RSDT").unwrap();
    const ENTRY_SIZE: usize = 4;
}

pub struct Extended;
// Safety: `Self::ENTRY_SIZE` is 4 (for XSDT).
unsafe impl RsdtKind for Extended {
    const SIGNATURE: AsciiStr<4> = AsciiStr::new(*b"XSDT").unwrap();
    const ENTRY_SIZE: usize = 8;
}

pub struct Rsdt<K> {
    base_ptr: NonNull<u8>,
    marker: PhantomData<K>,
}

// Safety: `Self::new` requires `self.0` be a valid base pointer.
unsafe impl<K: RsdtKind> SystemDescriptorTable for Rsdt<K> {
    const SIGNATURE: AsciiStr<4> = K::SIGNATURE;

    fn base_ptr(&self) -> NonNull<u8> {
        self.base_ptr
    }
}

impl<K> Rsdt<K> {
    pub unsafe fn new(base_ptr: NonNull<u8>) -> Self {
        Self {
            base_ptr,
            marker: PhantomData,
        }
    }
}

impl<K: RsdtKind> Rsdt<K> {
    pub fn entries(&self) -> RsdtIterator {
        RsdtIterator::new(self)
    }
}

impl<K: RsdtKind> core::fmt::Debug for Rsdt<K> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = f.debug_struct("Root System Descriptor Table");
        self.write_header_debug_fields(&mut d);
        d.finish()
    }
}
