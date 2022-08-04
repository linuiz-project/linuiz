mod portrw;

use core::marker::PhantomData;
use portrw::*;

pub trait PortRead {
    unsafe fn read(port: u16) -> Self;
}

pub trait PortWrite {
    unsafe fn write(port: u16, value: Self);
}

pub trait PortReadWrite: PortRead + PortWrite {}

/* PORTREAD */
impl PortRead for u8 {
    unsafe fn read(port: u16) -> Self {
        read8(port)
    }
}

impl PortRead for u16 {
    unsafe fn read(port: u16) -> Self {
        read16(port)
    }
}

impl PortRead for u32 {
    unsafe fn read(port: u16) -> Self {
        read32(port)
    }
}

/* PORTWRITE */
impl PortWrite for u8 {
    unsafe fn write(port: u16, value: Self) {
        write8(port, value)
    }
}

impl PortWrite for u16 {
    unsafe fn write(port: u16, value: Self) {
        write16(port, value)
    }
}

impl PortWrite for u32 {
    unsafe fn write(port: u16, value: Self) {
        write32(port, value)
    }
}

/* PORT RW */
impl PortReadWrite for u8 {}
impl PortReadWrite for u16 {}
impl PortReadWrite for u32 {}

/* READ ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadOnlyPort<T: PortRead> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortRead> ReadOnlyPort<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    pub const unsafe fn new(port: u16) -> Self {
        ReadOnlyPort { port, phantom: PhantomData }
    }

    pub const fn port_num(&self) -> u16 {
        self.port
    }

    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }
}

/* WRITE ONLY PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct WriteOnlyPort<T: PortWrite> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortWrite> WriteOnlyPort<T> {
    // TODO add a raw_write -esque function.

    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    pub const unsafe fn new(port: u16) -> Self {
        WriteOnlyPort { port, phantom: PhantomData }
    }

    pub const fn port_num(&self) -> u16 {
        self.port
    }

    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}

/* READ/WRITE PORT */
#[repr(transparent)]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ReadWritePort<T: PortReadWrite> {
    port: u16,
    phantom: PhantomData<T>,
}

impl<T: PortReadWrite> ReadWritePort<T> {
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    pub const unsafe fn new(port: u16) -> Self {
        ReadWritePort { port, phantom: PhantomData }
    }

    pub const fn port_num(&self) -> u16 {
        self.port
    }

    pub fn read(&self) -> T {
        unsafe { T::read(self.port_num()) }
    }

    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port_num(), value) }
    }
}

pub struct ParallelPort<T: PortReadWrite> {
    read: ReadOnlyPort<T>,
    write: WriteOnlyPort<T>,
}

impl<T: PortReadWrite> ParallelPort<T> {
    pub const unsafe fn new(read_addr: u16, write_addr: u16) -> Self {
        Self { read: ReadOnlyPort::new(read_addr), write: WriteOnlyPort::new(write_addr) }
    }

    pub fn write(&mut self, data: T) {
        self.write.write(data);
    }

    pub fn read(&self) -> T {
        self.read.read()
    }
}
