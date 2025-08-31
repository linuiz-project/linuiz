use crate::{acpi::SystemDescriptorTable, util::AsciiStr};
use core::{marker::PhantomData, ptr::NonNull};

mod iter;
pub use iter::*;

pub trait RsdtKind {
    const SIGNATURE: AsciiStr<4>;

    type Entry;
}

pub struct Standard;
impl RsdtKind for Standard {
    const SIGNATURE: AsciiStr<4> = AsciiStr::new(*b"RSDT").unwrap();
    type Entry = u32;
}

pub struct Extended;
impl RsdtKind for Extended {
    const SIGNATURE: AsciiStr<4> = AsciiStr::new(*b"XSDT").unwrap();
    type Entry = u64;
}

pub struct Rsdt<K> {
    base_ptr: NonNull<u8>,
    marker: PhantomData<K>,
}

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
