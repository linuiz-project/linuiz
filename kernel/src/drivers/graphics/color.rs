#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Color8i {
    pub b: u8,
    pub g: u8,
    pub r: u8,
    reserved: u8,
}

impl Color8i {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color8i {
            b,
            g,
            r,
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

#[repr(u8)]
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Colors {
    Black,
    White,
    Blue,
    Green,
    Red,
    Cyan,
    Magenta,
    Yellow,
    Brown,
    LightGrey,
    DarkGrey,
    LightBlue,
    LightGreen,
    LightCyan,
    LightRed,
    Pink,
}

impl From<Colors> for Color8i {
    fn from(value: Colors) -> Color8i {
        match value {
            Colors::Black => Color8i::new(0, 0, 0),
            Colors::White => Color8i::new(255, 255, 255),
            Colors::Blue => Color8i::new(0, 0, 255),
            Colors::Green => Color8i::new(0, 255, 0),
            Colors::Red => Color8i::new(255, 0, 0),
            Colors::Cyan => Color8i::new(0, 255, 255),
            Colors::Magenta => Color8i::new(255, 0, 255),
            Colors::Yellow => Color8i::new(255, 255, 0),
            Colors::Brown => Color8i::new(210, 105, 30),
            Colors::LightGrey => Color8i::new(211, 211, 211),
            Colors::DarkGrey => Color8i::new(169, 169, 169),
            Colors::LightBlue => Color8i::new(173, 126, 230),
            Colors::LightGreen => Color8i::new(144, 238, 144),
            Colors::LightCyan => Color8i::new(224, 255, 255),
            Colors::LightRed => Color8i::new(255, 204, 203),
            Colors::Pink => Color8i::new(255, 192, 203),
        }
    }
}
