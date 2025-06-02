mod frame;
mod page;
mod physical;
mod r#virtual;

pub use frame::*;
pub use page::*;
pub use physical::*;
pub use r#virtual::*;

use core::fmt;

pub trait Addressable: Sized {
    type Repr;
    type Init;
    type Get;

    const DEBUG_NAME: &'static str;

    fn new(init: Self::Init) -> Option<Self::Repr>;
    fn new_truncate(init: Self::Init) -> Self::Repr;

    fn get(repr: Self::Repr) -> Self::Get;
}

pub trait PtrAddressable: Addressable {
    fn from_ptr<T>(ptr: *mut T) -> Self::Repr;
    fn as_ptr(repr: Self::Repr) -> *mut u8;
}

pub trait IndexAddressable: Addressable {
    fn from_index(index: usize) -> Option<Self::Repr>;
    fn index(repr: Self::Repr) -> usize;
}

pub trait DefaultableAddressKind: Addressable {
    fn default() -> Self::Repr;
}

pub struct Address<Kind: Addressable>(Kind::Repr);

impl<Kind: Addressable> Address<Kind> {
    pub fn new(init: Kind::Init) -> Option<Self> {
        Kind::new(init).map(Self)
    }

    pub fn new_truncate(init: Kind::Init) -> Self {
        Self(Kind::new_truncate(init))
    }

    pub fn get(self) -> Kind::Get {
        Kind::get(self.0)
    }
}

impl<Kind: PtrAddressable> Address<Kind> {
    pub fn from_ptr<T>(ptr: *mut T) -> Self {
        Self(Kind::from_ptr(ptr))
    }

    pub fn as_ptr(self) -> *mut u8 {
        Kind::as_ptr(self.0)
    }
}

impl<Kind: IndexAddressable> Address<Kind> {
    pub fn from_index(index: usize) -> Option<Self> {
        Kind::from_index(index).map(Self)
    }

    pub fn index(self) -> usize {
        Kind::index(self.0)
    }
}

impl<Repr: Default, I, Kind: Addressable<Init = I, Repr = Repr>> Default for Address<Kind> {
    fn default() -> Self {
        Self(Repr::default())
    }
}

impl<Repr: Clone, I, Kind: Addressable<Init = I, Repr = Repr>> Clone for Address<Kind> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Repr: Copy, I, Kind: Addressable<Init = I, Repr = Repr>> Copy for Address<Kind> {}

impl<Repr: PartialEq, I, Kind: Addressable<Init = I, Repr = Repr>> PartialEq for Address<Kind> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<Repr: Eq, I, Kind: Addressable<Init = I, Repr = Repr>> Eq for Address<Kind> {}

impl<I, Repr: fmt::Debug, Kind: Addressable<Init = I, Repr = Repr>> fmt::Debug for Address<Kind> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple(Kind::DEBUG_NAME).field(&self.0).finish()
    }
}
