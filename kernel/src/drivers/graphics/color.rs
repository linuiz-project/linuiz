#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Color8i {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    reserved: u8,
}

impl Color8i {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color8i {
            r,
            g,
            b,
            reserved: 0x0,
        }
    }
}

impl From<u32> for Color8i {
    fn from(from: u32) -> Self {
        let r = ((from >> 24) & 0xFF) as u8;
        let g = ((from >> 16) & 0xFF) as u8;
        let b = ((from >> 8) & 0xFF) as u8;
        let reserved = (from & 0xFF) as u8;

        Self { r, g, b, reserved }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Color {
    Black,
    Gray,
}

impl Into<Color8i> for Color {
    fn into(self) -> Color8i {
        match self {
            Color::Black => Color8i::new(0, 0, 0),
            Color::Gray => Color8i::new(100, 100, 100),
        }
    }
}
