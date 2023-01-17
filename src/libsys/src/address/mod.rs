mod frame;
mod page;
mod physical;
mod virt;

pub use frame::*;
pub use page::*;
pub use physical::*;
pub use virt::*;

use core::fmt;

pub trait Addressable: Sized {
    type InitType;
    type ReprType: Copy;

    fn new(init: Self::InitType) -> Option<Self::ReprType>;
    fn new_truncate(init: Self::InitType) -> Self::ReprType;
}

pub trait PtrAddressable: Addressable {
    fn from_ptr<T>(ptr: *mut T) -> Self::ReprType;
    fn as_ptr(repr: Self::ReprType) -> *mut u8;
}

pub trait IndexAddressable: Addressable {
    fn from_index(index: usize) -> Option<Self::ReprType>;
    fn index(repr: Self::ReprType) -> usize;
}

pub trait DefaultableAddressKind: Addressable {
    fn default() -> Self::ReprType;
}

pub struct Address<Kind: Addressable>(Kind::ReprType);

impl<Kind: Addressable> Address<Kind> {
    pub fn new(init: Kind::InitType) -> Option<Self> {
        Kind::new(init).map(Self)
    }

    pub fn new_truncate(init: Kind::InitType) -> Self {
        Self(Kind::new_truncate(init))
    }

    pub fn get(self) -> Kind::ReprType {
        self.0
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

impl<Repr: Default, I, K: Addressable<InitType = I, ReprType = Repr>> Default for Address<K> {
    fn default() -> Self {
        Self(Repr::default())
    }
}

impl<Repr: Clone, I, K: Addressable<InitType = I, ReprType = Repr>> Clone for Address<K> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<Repr: Copy, I, K: Addressable<InitType = I, ReprType = Repr>> Copy for Address<K> {}

impl<Repr: PartialEq, I, K: Addressable<InitType = I, ReprType = Repr>> PartialEq for Address<K> {
    fn eq(&self, other: &Self) -> bool {
        self.0.eq(&other.0)
    }
}

impl<Repr: Eq, I, K: Addressable<InitType = I, ReprType = Repr>> Eq for Address<K> {}

impl<I, Repr: fmt::Debug, K: Addressable<InitType = I, ReprType = Repr>> fmt::Debug for Address<K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Address").field(&self.0).finish()
    }
}
