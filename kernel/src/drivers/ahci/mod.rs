pub mod hba;

use alloc::vec::Vec;
use bit_field::BitField;
use hba::{
    port::{HBAPort, HostBusAdapterPortClass},
    HBAMemory,
};
use libkernel::{
    io::pci::{PCIeDevice, Standard, StandardRegister},
    Address,
};

pub const ATA_DEV_BUSY: u8 = 0x80;
pub const ATA_DEV_DRQ: u8 = 0x08;
pub const ATA_CMD_READ_DMA_EX: u8 = 0x25;
pub const HBA_PxIS_TFES: u32 = 1 << 30;

#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum FISType {
    H2D = 0x27,
    D2H = 0x34,
    DMA_ACT = 0x39,
    DMA_SETUP = 0x41,
    DATA = 0x46,
    BIST = 0x48,
    PIO_SETUP = 0x5F,
    DEV_BITS = 0xA1,
}

#[repr(C)]
pub struct FIS_REG_H2D {
    fis_type: FISType,
    bits1: u8,
    command: u8,
    feature_low: u8,
    lba0: u8,
    lba1: u8,
    lba2: u8,
    device_register: u8,
    lba3: u8,
    lba4: u8,
    lba5: u8,
    feature_high: u8,
    count_low: u8,
    count_high: u8,
    iso_cmd_completion: u8,
    control: u8,
    rsvd0: [u8; 4],
}

impl FIS_REG_H2D {
    libkernel::bitfield_getter!(bits1, u8, port_multiplier, 0..4);
    libkernel::bitfield_getter!(bits1, command_control, 7);

    pub fn set_sector_base(&mut self, sector: usize) {
        assert_eq!(sector & 0xFFFFFFFFFFFF, 0, "sector is 48 bits");

        self.lba0 = sector.get_bits(0..8) as u8;
        self.lba1 = sector.get_bits(8..16) as u8;
        self.lba2 = sector.get_bits(16..24) as u8;
        self.lba3 = sector.get_bits(24..32) as u8;
        self.lba4 = sector.get_bits(32..40) as u8;
        self.lba5 = sector.get_bits(40..48) as u8;
    }

    pub fn set_sector_count(&mut self, sectors: u16) {
        self.count_low = sectors as u8;
        self.count_high = (sectors >> 8) as u8;
    }
}

impl self::hba::port::CommandFIS for FIS_REG_H2D {}

#[derive(Debug)]
pub struct AHCIPort<'hba> {
    port_num: u8,
    hba_port: &'hba mut HBAPort,
    buffer: [u8; 2048],
}

impl<'hba> AHCIPort<'hba> {
    fn new(port_num: u8, hba_port: &'hba mut HBAPort) -> Self {
        Self {
            port_num,
            hba_port,
            buffer: [0u8; 2048],
        }
    }

    pub fn hba(&mut self) -> &mut HBAPort {
        self.hba_port
    }

    pub fn start_cmd(&mut self) {
        debug!("AHCI PORT #{}: START CMD", self.port_num);

        let cmd = self.hba_port.command_status();

        while cmd.get_cr() {}

        cmd.set_fre(true);
        cmd.set_st(true);
    }

    pub fn stop_cmd(&mut self) {
        debug!("AHCI PORT #{}: STOP CMD", self.port_num);

        let cmd = self.hba_port.command_status();

        cmd.set_st(false);
        cmd.set_fre(false);

        while cmd.get_fr() | cmd.get_cr() {}
    }

    pub fn configure(&mut self) {
        debug!("AHCI PORT #{}: CONFIGURING", self.port_num);

        self.stop_cmd();

        debug!("Allocting command and FIS lists.");

        let cmd_base: *mut u8 = libkernel::alloc!(4096, 128);
        let fis_base: *mut u8 = libkernel::alloc!(1024, 128);

        unsafe {
            debug!("Clearing command and FIS lists, and updating port metadata.");

            core::ptr::write_bytes(cmd_base, 0, 4096);
            core::ptr::write_bytes(fis_base, 0, 1024);

            self.hba_port
                .set_command_list_addr(Address::from_ptr(cmd_base));
            self.hba_port.set_fis_addr(Address::from_ptr(fis_base));
        }

        debug!("Configuring individual command headers.");

        for (index, cmd_header) in self.hba_port.command_list().unwrap().iter_mut().enumerate() {
            *cmd_header.prdt_len() = 8;

            unsafe {
                let cmd_table_addr: *mut u8 = libkernel::alloc!(4096, 128);
                core::ptr::write_bytes(cmd_table_addr, 0, 4096);

                debug!(
                    "Configured command header #{} with table address {:?}",
                    index, cmd_table_addr
                );

                cmd_header.set_command_table_base_addr(libkernel::Address::<
                    libkernel::addr_ty::Virtual,
                >::from_ptr(cmd_table_addr))
            };
        }

        self.start_cmd();

        debug!("AHCI PORT #{}: CONFIGURED", self.port_num);
    }

    pub fn read(&mut self, sector_base: usize, sector_count: u16) -> Vec<u8> {
        //debug!("AHCI PORT #{}: READ RECEIVED", self.port_num);

        const MAX_SPIN: usize = 1000000;

        //debug!("AHCI PORT #{}: READ: BUSY WAIT", self.port_num);
        let mut spin: usize = 0;
        while (self.hba_port.task_file_data() & ((ATA_DEV_BUSY | ATA_DEV_DRQ) as u32)) > 0
            && spin < MAX_SPIN
        {
            spin += 1
        }

        if spin >= MAX_SPIN {
            panic!("failed to read from disk (busy)");
        }

        //debug!("AHCI PORT #{}: READ: CLR INT STATUS", self.port_num);
        self.hba_port.interrupt_status().clear(); // clear interrupts

        let cmd_headers = self
            .hba_port
            .command_list()
            .expect("AHCI port's HBA has not yet been configured");

        //debug!("AHCI PORT #{}: READ: CFG COMMAND HEADER", self.port_num);
        let cmd_header = &mut cmd_headers[1];
        cmd_header.set_fis_len(
            (core::mem::size_of::<FIS_REG_H2D>() / core::mem::size_of::<u32>()) as u16,
        );
        cmd_header.set_write(false);
        *cmd_header.prdt_len() = 1;

        //debug!("AHCI PORT #{}: READ: CFG COMMAND TABLE", self.port_num);
        let command_table = &mut cmd_header.command_tables().unwrap()[0];
        command_table.clear(1);

        //debug!("AHCI PORT #{}: READ: CFG PRDT ENTRY", self.port_num);
        let prdt_entry = &mut command_table.prdt_entries(1)[0];
        let buffer: Vec<u8> = alloc::vec![0; (sector_count as usize) << 9];
        prdt_entry.set_db_addr(Address::from_ptr(buffer.as_ptr()));
        prdt_entry.set_sector_count(sector_count as u32);

        // debug!(
        //     "AHCI PORT #{}: READ: CFG COMMAND FIS (DIS_REG_H2D)",
        //     self.port_num
        // );
        let command_fis = command_table.command_fis::<FIS_REG_H2D>();
        command_fis.fis_type = FISType::H2D;
        command_fis.set_command_control(true); // is command
        command_fis.command = ATA_CMD_READ_DMA_EX;
        command_fis.set_sector_base(sector_base);
        command_fis.device_register = 1 << 6; // LBA mode
        command_fis.set_sector_count(sector_count);

        //debug!("AHCI PORT #{}: READ: ISSUING COMMAND", self.port_num);
        self.hba_port.issue_command_slot(0);

        // debug!(
        //     "AHCI PORT #{}: READ: READ EXECUTING: BUSY WAIT",
        //     self.port_num
        // );
        while self.hba_port.check_command_slot(0) {
            if self.hba_port.interrupt_status().get_tfes() {
                panic!("read failed (HBA PxIS TFES")

                // TODO interrupt status register
            }
        }

        //debug!("AHCI PORT #{}: READ COMPLETE", self.port_num);
        buffer
    }
}

pub struct AHCI<'ahci> {
    ports: Vec<AHCIPort<'ahci>>,
}

impl<'ahci> AHCI<'ahci> {
    pub fn from_pcie_device(device: &'ahci PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        if let Some(mut hba_register) = device.get_register_locked(StandardRegister::Reg5) {
            // Allows this context to 'own' and move around values derived from HBA memory.
            let own_hba_memory = hba_register.mapped_addr().as_mut_ptr::<HBAMemory>();

            let ports = unsafe { hba_register.read_mut::<HBAMemory>(0).unwrap() }
                .ports()
                .iter()
                .enumerate()
                .filter_map(|(port_num, port)| match port.class() {
                    HostBusAdapterPortClass::SATA | HostBusAdapterPortClass::SATAPI => {
                        debug!("Configuring AHCI port #{}: {:?}", port_num, port.class());

                        // This is very unsafe, but it elides the borrow checker, thus allowing us to point to MMIO that's
                        //  TECHNICALLY owned by the `device`.
                        let own_port = unsafe { &mut ((*own_hba_memory).ports_mut()[port_num]) };
                        Some(AHCIPort::new(port_num as u8, own_port))
                    }
                    _port_type => None,
                })
                .collect();

            Self { ports }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn iter(&'ahci self) -> core::slice::Iter<AHCIPort<'ahci>> {
        self.ports.iter()
    }

    pub fn iter_mut(&'ahci mut self) -> core::slice::IterMut<AHCIPort<'ahci>> {
        self.ports.iter_mut()
    }
}
