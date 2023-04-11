#![no_std]

mod portrw;

use core::marker::PhantomData;
pub use portrw::*;

/* PORT RW */
impl PortReadWrite for u8 {}
impl PortReadWrite for u16 {}
impl PortReadWrite for u32 {}

/* READ ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadOnlyPort<T: PortRead> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortRead> ReadOnlyPort<T> {
    /// Constructs a r/o port pointing to the provided address.
    ///
    /// ### Safety
    ///
    /// Providing an invalid port address will result in reading and writing to an invalid address.
    #[inline]
    pub const unsafe fn new(port: PortAddress) -> Self {
        ReadOnlyPort { port, phantom: PhantomData }
    }

    #[inline]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline]
    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }
}

/* WRITE ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WriteOnlyPort<T: PortWrite> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortWrite> WriteOnlyPort<T> {
    /// Constructs a w/o port pointing to the provided address.
    ///
    /// ### Safety
    ///
    /// Providing an invalid port address will result in reading and writing to an invalid address.
    #[inline]
    pub const unsafe fn new(port: PortAddress) -> Self {
        WriteOnlyPort { port, phantom: PhantomData }
    }

    #[inline]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline]
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}

/* READ/WRITE PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadWritePort<T: PortReadWrite> {
    port: PortAddress,
    phantom: PhantomData<T>,
}

impl<T: PortReadWrite> ReadWritePort<T> {
    /// Constructs a r/w port pointing to the provided address.
    ///
    /// ### Safety
    ///
    /// Providing an invalid port address will result in reading and writing to an invalid address.
    #[inline]
    pub const unsafe fn new(port: PortAddress) -> Self {
        ReadWritePort { port, phantom: PhantomData }
    }

    #[inline]
    pub const fn port_num(&self) -> PortAddress {
        self.port
    }

    #[inline]
    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }

    #[inline]
    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}
