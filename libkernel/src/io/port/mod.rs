mod portrw;

use core::marker::PhantomData;
use portrw::{PortRead, PortReadWrite, PortWrite};

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
        ReadOnlyPort {
            port,
            phantom: PhantomData,
        }
    }

    pub fn port_num(&self) -> u16 {
        self.port
    }

    pub fn read(&self) -> T {
        unsafe { T::read(self.port) }
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
    /// Constructs a port wrapping the given address
    ///
    /// This method is unsafe because the caller must ensure the given port is a valid address
    pub const unsafe fn new(port: u16) -> Self {
        WriteOnlyPort {
            port,
            phantom: PhantomData,
        }
    }

    pub fn port_num(&self) -> u16 {
        self.port
    }

    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port, value) }
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
        ReadWritePort {
            port,
            phantom: PhantomData,
        }
    }

    pub fn port_num(&self) -> u16 {
        self.port
    }

    pub fn read(&self) -> T {
        unsafe { T::read(self.port) }
    }

    pub fn write(&mut self, value: T) {
        unsafe { T::write(self.port, value) }
    }
}

pub struct ParallelPort<T: PortReadWrite> {
    read: ReadOnlyPort<T>,
    write: WriteOnlyPort<T>,
}

impl<T: PortReadWrite> ParallelPort<T> {
    pub const unsafe fn new(read_addr: u16, write_addr: u16) -> Self {
        Self {
            read: ReadOnlyPort::new(read_addr),
            write: WriteOnlyPort::new(write_addr),
        }
    }

    pub fn write(&mut self, data: T) {
        self.write.write(data);
    }

    pub fn read(&self) -> T {
        self.read.read()
    }
}
