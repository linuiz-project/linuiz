use crate::{
    acpi::{
        SystemDescriptorTable,
        fadt::Fadt,
        rsdt::{Rsdt, RsdtKind},
        waet::Waet,
    },
    mem::HigherHalfDirectMap,
    util::AsciiStr,
};
use core::{marker::PhantomData, ptr::NonNull};

#[derive(Debug)]
pub enum SdtVariant {
    Fadt(Fadt),
    Waet(Waet),

    Unknown(AsciiStr<4>),
}

pub struct RsdtIterator<'a> {
    ptr: NonNull<u8>,
    entry_size: usize,
    offset: usize,
    length: usize,
    marker: PhantomData<&'a ()>,
}

impl<'a> RsdtIterator<'a> {
    pub(super) fn new<K: RsdtKind>(rsdt: &'a Rsdt<K>) -> Self {
        Self {
            ptr: rsdt.base_ptr,
            entry_size: size_of::<K::Entry>(),
            offset: 36,
            length: rsdt.length(),
            marker: PhantomData,
        }
    }
}

impl Iterator for RsdtIterator<'_> {
    type Item = SdtVariant;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset > (self.length - self.entry_size) {
            return None;
        }

        let entry = {
            let ptr = unsafe { self.ptr.byte_add(self.offset) };

            match self.entry_size {
                4 => {
                    let address = unsafe { ptr.cast::<u32>().read_unaligned() };
                    usize::try_from(address).unwrap()
                }

                8 => {
                    let address = unsafe { ptr.cast::<u64>().read_unaligned() };
                    usize::try_from(address).unwrap()
                }

                _ => unreachable!(),
            }
        };

        self.offset += self.entry_size;

        let sdt_address = usize::try_from(entry).unwrap();
        let sdt_address = HigherHalfDirectMap::offset(sdt_address);
        let sdt_ptr = NonNull::<u8>::with_exposed_provenance(sdt_address);

        let sdt_signature = unsafe { sdt_ptr.cast::<[u8; 4]>().read_unaligned() };
        match AsciiStr::new_lossy(sdt_signature) {
            <Fadt as SystemDescriptorTable>::SIGNATURE => {
                let fadt = unsafe { Fadt::new(sdt_ptr) };
                Some(SdtVariant::Fadt(fadt))
            }

            <Waet as SystemDescriptorTable>::SIGNATURE => {
                let waet = unsafe { Waet::new(sdt_ptr) };
                Some(SdtVariant::Waet(waet))
            }

            unknown => Some(SdtVariant::Unknown(unknown)),
        }
    }
}
