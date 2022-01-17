#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GUID {
    a: u32,
    b: u16,
    c: u16,
    d: [u8; 8],
}

impl GUID {
    pub const fn new(
        time_low: u32,
        time_mid: u16,
        time_high_and_version: u16,
        clock_seq_and_variant: u16,
        node: [u8; 6],
    ) -> Self {
        Self {
            a: time_low,
            b: time_mid,
            c: time_high_and_version,
            d: [
                (clock_seq_and_variant / 0x100) as u8,
                (clock_seq_and_variant % 0x100) as u8,
                node[0],
                node[1],
                node[2],
                node[3],
                node[4],
                node[5],
            ],
        }
    }
}
