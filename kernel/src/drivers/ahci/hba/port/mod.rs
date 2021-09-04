mod status;

use libkernel::{
    addr_ty::Virtual, bitfield_getter, volatile::VolatileCell, volatile_bitfield_getter, Address,
    ReadOnly, ReadWrite,
};
use num_enum::TryFromPrimitive;

pub use status::*;

#[repr(C)]
pub struct HBAPRDTEntry {
    db_addr_lower: VolatileCell<u32, ReadWrite>,
    db_addr_upper: VolatileCell<u32, ReadWrite>,
    rsvd0: u32,
    bits: VolatileCell<u32, ReadWrite>,
}

impl HBAPRDTEntry {
    volatile_bitfield_getter!(bits, u32, byte_count, 0..22);
    volatile_bitfield_getter!(bits, interrupt_on_completion, 31);

    pub fn set_db_addr(&mut self, addr: libkernel::Address<libkernel::addr_ty::Virtual>) {
        let addr_usize = addr.as_usize();

        self.db_addr_lower.write(addr_usize as u32);
        self.db_addr_upper.write((addr_usize >> 32) as u32);
    }

    pub fn set_sector_count(&mut self, sector_count: u32) {
        self.set_byte_count(
            (sector_count << 9) - 1, /* 512-byte alignment per sector */
        );
    }

    pub fn clear(&mut self) {
        self.db_addr_lower.write(0);
        self.db_addr_upper.write(0);
        self.bits.write(0);
    }
}

pub trait CommandFIS {}

#[repr(C)]
pub struct HBACommandTable {
    command_fis: [u8; 64],
    atapi_command: [u8; 16],
    rsvd0: [u8; 48],
    prdt: core::ffi::c_void,
}

impl HBACommandTable {
    pub fn prdt_entries(&mut self, entry_count: u16) -> &mut [HBAPRDTEntry] {
        unsafe {
            core::slice::from_raw_parts_mut(
                (&mut self.prdt) as *mut _ as *mut HBAPRDTEntry,
                entry_count as usize,
            )
        }
    }

    pub fn clear(&mut self, prdt_entry_count: u16) {
        self.command_fis.fill(0);
        self.atapi_command.fill(0);
        self.prdt_entries(prdt_entry_count)
            .iter_mut()
            .for_each(|entry| entry.clear());
    }

    pub fn command_fis<T: CommandFIS>(&mut self) -> &mut T {
        unsafe { &mut *(self.command_fis.as_mut_ptr() as *mut _) }
    }
}

#[repr(C)]
pub struct HBACommandHeader {
    bits: u16,
    prdt_len: u16,
    prdb_count: u32,
    cmd_tbl_addr_lower: u32,
    cmd_tbl_addr_upper: u32,
    reserved1: [u8; 4],
}

impl HBACommandHeader {
    bitfield_getter!(bits, u16, fis_len, 0..5);
    bitfield_getter!(bits, atapi, 5);
    bitfield_getter!(bits, write, 6);
    bitfield_getter!(bits, prefetchable, 7);
    bitfield_getter!(bits, reset, 8);
    bitfield_getter!(bits, bist, 9);
    bitfield_getter!(bits, clear_busy_on_rok, 10);
    bitfield_getter!(bits, u16, port_multiplier, 12..16);

    pub fn prdt_len(&mut self) -> &mut u16 {
        &mut self.prdt_len
    }

    pub unsafe fn set_command_table_base_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();

        self.cmd_tbl_addr_lower = addr_usize as u32;
        self.cmd_tbl_addr_upper = (addr_usize >> 32) as u32;
    }

    pub fn command_table(&mut self) -> Option<&mut HBACommandTable> {
        if self.cmd_tbl_addr_lower > 0 || self.cmd_tbl_addr_upper > 0 {
            Some(unsafe {
                let lower = self.cmd_tbl_addr_lower as usize;
                let upper = (self.cmd_tbl_addr_upper as usize) << 32;

                &mut *((lower | upper) as *mut _)
            })
        } else {
            None
        }
    }
}

#[derive(Debug)]
pub enum HostBusAdapterPortClass {
    None,
    SATA,
    SEMB,
    PM,
    SATAPI,
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum DeviceDetectionInitialization {
    None = 0,
    FullReinit = 1,
    DisbaleSATA = 4,
}

#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum SpeedAllowed {
    NoRestriction = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
}

// IPWM = Interface Power Management
#[repr(u32)]
#[derive(TryFromPrimitive)]
pub enum IPWMTransitionsAllowed {
    NoRestriction = 0,
    PartialStateDisabled = 1,
    SlumberStateDisabled = 2,
    PartialAndSlumberStateDisabled = 3,
    DevSleepPWMStateDisabled = 4,
    PartialAndDevSleepPWNDisabled = 5,
    SlumberAndDevSleepPWMDisabled = 6,
    AllDisabled = 7,
}


// INVARIANT: All memory accesses through this struct are volatile.
#[repr(transparent)]
pub struct HBAPortCommandStatus {
    bits: VolatileCell<u32, ReadWrite>,
}

impl HBAPortCommandStatus {
    volatile_bitfield_getter!(bits, st, 0);
    // SUD - Check CAP.SSS is 1 or 0 for RW or RO

    volatile_bitfield_getter_ro!(bits, pod, 2);
    pub fn set_pod(&mut self, set: bool) -> Result<(), ()> {
        if self.get_cpd() {
            self.bits.write(*(self.bits.read().set_bit(2, set)));

            Ok(())
        } else {
            Err(())
        }
    }

    volatile_bitfield_getter!(bits, clo, 3);
    volatile_bitfield_getter!(bits, fre, 4);
    volatile_bitfield_getter_ro!(bits, u32, ccs, 8..13);
    volatile_bitfield_getter_ro!(bits, mpss, 13);
    volatile_bitfield_getter_ro!(bits, fr, 14);
    volatile_bitfield_getter_ro!(bits, cr, 15);
    volatile_bitfield_getter_ro!(bits, cps, 16);
    // PMA - check CAP.SPM = 1 or 0 for RW or RO
    volatile_bitfield_getter_ro!(bits, hpcp, 18);
    volatile_bitfield_getter_ro!(bits, mpsp, 19);
    volatile_bitfield_getter_ro!(bits, cpd, 20);
    volatile_bitfield_getter_ro!(bits, esp, 21);
    volatile_bitfield_getter_ro!(bits, fbscp, 22);
    volatile_bitfield_getter!(bits, apste, 22);
    volatile_bitfield_getter!(bits, atapi, 24);
    volatile_bitfield_getter!(bits, dlae, 25);
    // ALPE - Check CAP.SALP is 1 or 0 for RW or Reserved
    // ASP - Check CAP.SALP is 1 or 0 for RW or Reserved
    volatile_bitfield_getter!(bits, u32, icc, 28..32);
}

impl core::fmt::Debug for HBAPortCommandStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Command Status Register")
            .field("ST", &self.get_st())
            // .field("SUD", &self.sud())
            .field("CLO", &self.get_clo())
            .field("FRE", &self.get_fre())
            // .field("CCS", &self.ccs())
            .field("MPSS", &self.get_mpss())
            .field("FR", &self.get_fr())
            .field("CR", &self.get_cr())
            .field("CPS", &self.get_cps())
            // .field("PMA", &self.pma())
            .field("HPCP", &self.get_hpcp())
            .field("MPSP", &self.get_mpsp())
            .field("CPD", &self.get_cpd())
            .field("ESP", &self.get_esp())
            .field("APSTE", &self.get_apste())
            .field("ATAPI", &self.get_atapi())
            .field("DLAE", &self.get_dlae())
            // .field("ALPE", &self.alpe())
            // .field("ASP", &self.asp())
            // .field("ICC", &self.icc())
            .finish()
    }
}

// INVARIANT: All memory accesses through this struct are volatile.
#[repr(transparent)]
pub struct HBAPortInterruptStatus {
    bits: VolatileCell<u32, ReadWrite>,
}

impl HBAPortInterruptStatus {
    volatile_bitfield_getter!(bits, dhrs, 0);
    volatile_bitfield_getter!(bits, pss, 1);
    volatile_bitfield_getter!(bits, dss, 2);
    volatile_bitfield_getter!(bits, sdbs, 3);
    volatile_bitfield_getter_ro!(bits, ufs, 4);
    volatile_bitfield_getter!(bits, dps, 5);
    volatile_bitfield_getter_ro!(bits, pcs, 6);
    volatile_bitfield_getter!(bits, dmps, 7);
    volatile_bitfield_getter_ro!(bits, prcs, 22);
    volatile_bitfield_getter!(bits, ipms, 23);
    volatile_bitfield_getter!(bits, ofs, 24);
    volatile_bitfield_getter!(bits, infs, 26);
    volatile_bitfield_getter!(bits, ifs, 27);
    volatile_bitfield_getter!(bits, hbds, 28);
    volatile_bitfield_getter!(bits, hbfs, 29);
    volatile_bitfield_getter!(bits, tfes, 30);
    volatile_bitfield_getter!(bits, cpds, 31);

    pub fn clear(&mut self) {
        self.bits.write(0);
    }
}

impl core::fmt::Debug for HBAPortInterruptStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Interrupt Status")
            .field("DHRS", &self.get_dhrs())
            .field("PSS", &self.get_pss())
            .field("DSS", &self.get_dss())
            .field("UFS", &self.get_ufs())
            .field("DPS", &self.get_dps())
            .field("PCS", &self.get_pcs())
            .field("DMPS", &self.get_dmps())
            .field("PRCS", &self.get_prcs())
            .field("IPMS", &self.get_ipms())
            .field("OFS", &self.get_ofs())
            .field("INFS", &self.get_infs())
            .field("IFS", &self.get_ifs())
            .field("HBDS", &self.get_hbds())
            .field("HBFS", &self.get_hbfs())
            .field("TFES", &self.get_tfes())
            .field("CPDS", &self.get_cpds())
            .finish()
    }
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum InterfacePowerManagement {
    NonCommunicate = 0,
    Active = 1,
    Partial = 2,
    Slumber = 6,
    DevSleep = 8,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum InterfaceSpeed {
    NonCommunicate = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum DeviceDetection {
    NonCommunicate = 0,
    DetectedNoPhy = 1,
    DetectedAndPhy = 3,
    PhyOffline = 4,
}

// INVARIANT: All memory accesses through this struct are volatile.
#[repr(transparent)]
pub struct HBAPortSATAStatus {
    status: VolatileCell<u32, ReadWrite>,
}

impl HBAPortSATAStatus {
    pub fn interface_pwm(&self) -> InterfacePowerManagement {
        InterfacePowerManagement::try_from(self.status.read().get_bits(8..12)).unwrap()
    }

    pub fn interface_speed(&self) -> InterfaceSpeed {
        InterfaceSpeed::try_from(self.status.read().get_bits(4..8)).unwrap()
    }

    pub fn device_detection(&self) -> DeviceDetection {
        DeviceDetection::try_from(self.status.read().get_bits(0..4)).unwrap()
    }
}

impl core::fmt::Debug for HBAPortSATAStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SATA Port Status")
            .field("Interface PWM", &self.interface_pwm())
            .field("Interface Speed", &self.interface_speed())
            .field("Device Detection", &self.device_detection())
            .finish()
    }
}


// INVARIANT: All memory accesses through this struct are volatile.
#[repr(C)]
pub struct HBAPort {
    pub cmd_list_addr_lower: VolatileCell<u32, ReadWrite>,
    cmd_list_addr_upper: VolatileCell<u32, ReadWrite>,
    fis_addr_lower: VolatileCell<u32, ReadWrite>,
    fis_addr_upper: VolatileCell<u32, ReadWrite>,
    interrupt_status: HBAPortInterruptStatus,
    interrupt_enable: VolatileCell<u32, ReadOnly>,
    command_status: HBAPortCommandStatus,
    _reserved0: [u8; 4],
    task_file_data: VolatileCell<u32, ReadOnly>,
    signature: VolatileCell<u32, ReadOnly>,
    sata_status: HBAPortSATAStatus,
    sata_control: VolatileCell<u32, ReadOnly>,
    sata_error: VolatileCell<u32, ReadOnly>,
    sata_active: VolatileCell<u32, ReadOnly>,
    command_issue: VolatileCell<u32, ReadWrite>,
    sata_notification: VolatileCell<u32, ReadOnly>,
    fis_switch_control: VolatileCell<u32, ReadOnly>,
    _reserved1: [u8; 11],
    _vendor0: [u8; 4],
}

impl HBAPort {
    pub fn class(&self) -> HostBusAdapterPortClass {
        // Ensures port is in a valid state (deteced & powered).
        if !matches!(
            self.sata_status().device_detection(),
            DeviceDetection::DetectedAndPhy
        ) || !matches!(
            self.sata_status().interface_pwm(),
            InterfacePowerManagement::Active
        ) {
            HostBusAdapterPortClass::None
        } else {
            // Finally, determine port type from its signature.
            match self.signature.read() {
                0x00000101 => HostBusAdapterPortClass::SATA,
                0xC33C0101 => HostBusAdapterPortClass::SEMB,
                0x96690101 => HostBusAdapterPortClass::PM,
                0xEB140101 => HostBusAdapterPortClass::SATAPI,
                signature => panic!("invalid signature: {}", signature),
            }
        }
    }

    pub unsafe fn set_command_list_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();
        let lower = addr_usize as u32;
        let upper = (addr_usize >> 32) as u32;

        debug!(
            "SET CMD LIST ADDR: {:?}\n\tUPPER {}\n\tLOWER {}",
            addr, upper, lower
        );

        self.cmd_list_addr_upper.write(upper);
        self.cmd_list_addr_lower.write(lower);
    }

    pub unsafe fn set_fis_addr(&mut self, addr: Address<Virtual>) {
        let addr_usize = addr.as_usize();
        let lower = addr_usize as u32;
        let upper = (addr_usize >> 32) as u32;

        debug!(
            "SET FIS ADDR: {:?}\n\tUPPER {}\n\tLOWER {}",
            addr, upper, lower
        );

        self.fis_addr_upper.write(upper);
        self.fis_addr_lower.write(lower);
    }

    pub fn sata_status(&self) -> &HBAPortSATAStatus {
        &self.sata_status
    }

    pub fn command_status(&mut self) -> &mut HBAPortCommandStatus {
        &mut self.command_status
    }

    pub fn task_file_data(&self) -> u32 {
        self.task_file_data.read()
    }

    pub fn command_list(&mut self) -> Option<&mut [HBACommandHeader]> {
        if self.cmd_list_addr_lower.read() > 0 || self.cmd_list_addr_upper.read() > 0 {
            Some(unsafe {
                let lower = self.cmd_list_addr_lower.read() as usize;
                let upper = (self.cmd_list_addr_upper.read() as usize) << 32;

                core::slice::from_raw_parts_mut((upper | lower) as *mut _, 32)
            })
        } else {
            None
        }
    }

    pub fn interrupt_status(&mut self) -> &mut HBAPortInterruptStatus {
        &mut self.interrupt_status
    }

    pub fn issue_command_slot(&mut self, cmd_index: usize) {
        assert!(cmd_index < 32, "Command index must be between 0..32");
        assert!(
            self.command_status().get_st(),
            "CMD.ST bit must be set for a command to be issued"
        );

        self.command_issue
            .write(self.command_issue.read() | (1 << cmd_index));
    }

    pub fn check_command_slot(&mut self, cmd_index: usize) -> bool {
        assert!(cmd_index < 32, "Command index must be between 0..32");

        use bit_field::BitField;
        self.command_issue.read().get_bit(cmd_index)
    }

    pub fn start_cmd(&mut self) {
        debug!("AHCI PORT: START CMD");

        let cmd = self.command_status();

        while cmd.get_cr() {}

        cmd.set_fre(true);
        cmd.set_st(true);
    }

    pub fn stop_cmd(&mut self) {
        debug!("AHCI PORT: STOP CMD");

        let cmd = self.command_status();

        cmd.set_st(false);
        cmd.set_fre(false);

        while cmd.get_fr() | cmd.get_cr() {}
    }

    pub fn configure(&mut self) {
        self.stop_cmd();
        debug!("AHCI PORT: CONFIGURING");

        debug!("Allocting command and FIS lists.");

        let cmd_base: *mut u8 = libkernel::alloc!(4096, 128);
        debug!("\tCommand list base address: {:?}", cmd_base);
        let fis_base: *mut u8 = libkernel::alloc!(1024, 128);
        debug!("\tFIS base address: {:?}", fis_base);

        unsafe {
            debug!("Clearing command and FIS lists, and updating port metadata.");

            core::ptr::write_bytes(cmd_base, 0, 4096);
            core::ptr::write_bytes(fis_base, 0, 1024);

            self.set_command_list_addr(Address::from_ptr(cmd_base));
            self.set_fis_addr(Address::from_ptr(fis_base));
        }

        debug!("Configuring individual command headers.");

        for (index, cmd_header) in self.command_list().unwrap().iter_mut().enumerate() {
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

        debug!("AHCI PORT: CONFIGURED");
        self.start_cmd();
    }

    pub fn read(&mut self, sector_base: usize, sector_count: u16) -> alloc::vec::Vec<u8> {
        use crate::drivers::ahci::{
            FISType, ATA_CMD_READ_DMA_EX, ATA_DEV_BUSY, ATA_DEV_DRQ, FIS_REG_H2D,
        };

        debug!("AHCI PORT: READ: RECEIVED");

        const MAX_SPIN: usize = 1000000;

        debug!("AHCI PORT: READ: BUSY WAIT");
        let mut spin: usize = 0;
        while (self.task_file_data() & ((ATA_DEV_BUSY | ATA_DEV_DRQ) as u32)) > 0 && spin < MAX_SPIN
        {
            spin += 1
        }

        if spin >= MAX_SPIN {
            panic!("failed to read from disk (busy)");
        }

        debug!("AHCI PORT: READ: CLR INT STATUS");
        self.interrupt_status().clear(); // clear interrupts

        let cmd_headers = self
            .command_list()
            .expect("AHCI port's HBA has not yet been configured");

        debug!("AHCI PORT: READ: CFG COMMAND HEADER");
        let cmd_header = &mut cmd_headers[0];
        cmd_header.set_fis_len(
            (core::mem::size_of::<FIS_REG_H2D>() / core::mem::size_of::<u32>()) as u16,
        );
        cmd_header.set_write(false);
        *cmd_header.prdt_len() = 1;

        debug!("AHCI PORT: READ: CFG COMMAND TABLE");
        let command_table = &mut cmd_header.command_tables().unwrap()[0];
        command_table.clear(1);

        debug!("AHCI PORT: READ: CFG PRDT ENTRY");
        let prdt_entry = &mut command_table.prdt_entries(1)[0];
        let buffer: alloc::vec::Vec<u8> = alloc::vec![0; (sector_count as usize) << 9];
        prdt_entry.set_db_addr(Address::from_ptr(buffer.as_ptr()));
        prdt_entry.set_sector_count(sector_count as u32);

        debug!("AHCI PORT: READ: CFG COMMAND FIS (DIS_REG_H2D)");
        let command_fis = command_table.command_fis::<FIS_REG_H2D>();
        command_fis.fis_type = FISType::H2D;
        command_fis.set_command_control(true); // is command
        command_fis.command = ATA_CMD_READ_DMA_EX;
        command_fis.set_sector_base(sector_base);
        command_fis.device_register = 1 << 6; // LBA mode
        command_fis.set_sector_count(sector_count);

        debug!("AHCI PORT: READ: ISSUING COMMAND",);
        self.issue_command_slot(0);

        debug!("AHCI PORT: READ: READ EXECUTING: BUSY WAIT",);
        while self.check_command_slot(0) {
            if self.interrupt_status().get_tfes() {
                panic!("read failed (HBA PxIS TFES");

                // TODO interrupt status register
            }
        }

        debug!("AHCI PORT: READ: COMPLETE");
        buffer
    }
}
