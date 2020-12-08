#[repr(u32)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum SectionHeaderType {
    // todo this
}


#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct SectionHeader {
    name: u32,
    sh_type: u32,
    // todo fill this out
}