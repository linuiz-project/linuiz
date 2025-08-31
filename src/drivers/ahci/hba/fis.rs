use crate::drivers::ahci::CommandType;

#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum Type {
    None = 0x0,
    Hw2Dev = 0x27,
    Dev2Hw = 0x34,
    DMA_ACT = 0x39,
    DMA_SETUP = 0x41,
    DATA = 0x46,
    BIST = 0x48,
    PIO_SETUP = 0x5F,
    DEV_BITS = 0xA1,
}

#[repr(C)]
pub struct Hw2Dev {
    ty: Type,
    bits1: u8,
    command: CommandType,
    feature_low: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_high: u8,
    count: u16,
    iso_cmd_compl: u8,
    control: u8,
    rsvd0: [u8; 4],
}

impl Hw2Dev {
    libsys::bitfield_getter!(bits1, u8, port_multiplier, 0..4);
    libsys::bitfield_getter!(bits1, command_control, 7);

    pub const fn read(sector_base: usize, sector_count: u16) -> Self {
        Self {
            ty: Type::Hw2Dev,
            bits1: 1 << 7,
            command: CommandType::ReadDMA,
            feature_low: 0,
            lba0: (sector_base >> 0) as u8,
            lba1: (sector_base >> 8) as u8,
            lba2: (sector_base >> 16) as u8,
            device: 1 << 6,
            lba3: (sector_base >> 24) as u8,
            lba4: (sector_base >> 32) as u8,
            lba5: (sector_base >> 40) as u8,
            feature_high: 0,
            count: sector_count,
            iso_cmd_compl: 0,
            control: 0,
            rsvd0: [0u8; 4],
        }
    }

    pub fn set_sector_base(&mut self, sector: usize) {
        assert_eq!(sector & 0xFFFF_FFFF_FFFF, 0, "`sector` is a 48");

        self.lba0 = (sector >> 0) as u8;
        self.lba1 = (sector >> 8) as u8;
        self.lba2 = (sector >> 16) as u8;
        self.lba3 = (sector >> 24) as u8;
        self.lba4 = (sector >> 32) as u8;
        self.lba5 = (sector >> 40) as u8;
    }

    pub fn set_sector_count(&mut self, sector_count: u16) {
        self.count = sector_count;
    }
}

impl super::super::hba::CommandFIS for Hw2Dev {}
