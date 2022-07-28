#[repr(u64)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ID {
    Test,
}

#[repr(C)]
#[derive(Debug)]
pub struct Control {
    pub id: ID,
    pub blah: u64,
}

#[repr(u64)]
pub enum Error {
    ControlNotMapped,
}


