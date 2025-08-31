use bit_field::BitField;
use core::convert::TryFrom;
use libsys::{
    Address, ReadOnly, ReadWrite,
    memory::{Volatile, VolatileCell, VolatileSplitPtr},
    volatile_bitfield_getter, volatile_bitfield_getter_ro,
};
use num_enum::TryFromPrimitive;

// INVARIANT: All memory accesses through this struct are volatile.
#[repr(transparent)]
pub struct CommandStatus {
    bits: VolatileCell<u32, ReadWrite>,
}

impl CommandStatus {
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

impl Volatile for CommandStatus {}

impl core::fmt::Debug for CommandStatus {
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

#[repr(transparent)]
pub struct InterruptStatus {
    bits: VolatileCell<u32, ReadWrite>,
}

impl InterruptStatus {
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

    pub fn clear(&self) {
        self.bits.write(0);
    }
}

impl Volatile for InterruptStatus {}

impl core::fmt::Debug for InterruptStatus {
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
#[derive(Debug, PartialEq, Eq, TryFromPrimitive)]
pub enum InterfacePowerManagement {
    NonCommunicate = 0,
    Active = 1,
    Partial = 2,
    Slumber = 6,
    DevSleep = 8,
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, TryFromPrimitive)]
pub enum InterfaceSpeed {
    NonCommunicate = 0,
    Gen1 = 1,
    Gen2 = 2,
    Gen3 = 3,
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq, TryFromPrimitive)]
pub enum DeviceDetection {
    NonCommunicate = 0,
    DetectedNoPhy = 1,
    DetectedAndPhy = 3,
    PhyOffline = 4,
}

// INVARIANT: All memory accesses through this struct are volatile.
#[repr(transparent)]
pub struct SATAStatus {
    status: VolatileCell<u32, ReadWrite>,
}

impl SATAStatus {
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

impl Volatile for SATAStatus {}

impl core::fmt::Debug for SATAStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SATA Port Status")
            .field("Interface PWM", &self.interface_pwm())
            .field("Interface Speed", &self.interface_speed())
            .field("Device Detection", &self.device_detection())
            .finish()
    }
}

#[repr(u32)]
#[derive(Debug, PartialEq, Eq)]
pub enum Class {
    None = 0x0,
    SATA = 0x00000101,
    SEMB = 0xC33C0101,
    PM = 0x96690101,
    SATAPI = 0xEB140101,
}

// INVARIANT: All memory accesses through this struct are volatile.
#[repr(C)]
pub struct Port {
    cmd_list: VolatileSplitPtr<super::Command>,
    fis_addr: VolatileSplitPtr<u8>,
    interrupt_status: InterruptStatus,
    interrupt_enable: VolatileCell<u32, ReadOnly>,
    command_status: CommandStatus,
    _0: [u8; 4],
    task_file_data: VolatileCell<u32, ReadOnly>,
    signature: VolatileCell<Class, ReadOnly>,
    sata_status: SATAStatus,
    sata_control: VolatileCell<u32, ReadOnly>,
    sata_error: VolatileCell<u32, ReadOnly>,
    sata_active: VolatileCell<u32, ReadOnly>,
    command_issue: VolatileCell<u32, ReadWrite>,
    sata_notification: VolatileCell<u32, ReadOnly>,
    fis_switch_control: VolatileCell<u32, ReadOnly>,
    _1: [u8; 11],
    _vendor0: [u8; 4],
}

impl Port {
    pub fn class(&self) -> Class {
        // Ensures port is in a valid state (deteced & powered).
        if self.sata_status().device_detection() == DeviceDetection::DetectedAndPhy
            && self.sata_status().interface_pwm() == InterfacePowerManagement::Active
        {
            self.signature.read()
        } else {
            // Finally, determine port type from its signature.
            Class::None
        }
    }

    pub fn sata_status(&self) -> &SATAStatus {
        &self.sata_status
    }

    pub fn command_status(&self) -> &CommandStatus {
        &self.command_status
    }

    pub fn task_file_data(&self) -> u32 {
        self.task_file_data.read()
    }

    pub fn command_list(&self) -> &mut [super::Command] {
        unsafe { core::slice::from_raw_parts_mut(self.cmd_list.get_mut_ptr(), 32) }
    }

    pub fn interrupt_status(&self) -> &InterruptStatus {
        &self.interrupt_status
    }

    pub fn issue_command_slot(&self, cmd_index: usize) {
        assert!(cmd_index < 32, "Command index must be between 0..32");
        assert!(
            self.command_status().get_st(),
            "CMD.ST bit must be set for a command to be issued"
        );

        self.command_issue
            .write(self.command_issue.read() | (1 << cmd_index));
    }

    pub fn check_command_slot(&self, cmd_index: usize) -> bool {
        assert!(cmd_index < 32, "Command index must be between 0..32");

        self.command_issue.read().get_bit(cmd_index)
    }

    pub fn start_cmd(&self) {
        debug!("AHCI PORT: START CMD");

        let cmd = self.command_status();

        while cmd.get_cr() {}

        cmd.set_fre(true);
        cmd.set_st(true);
    }

    pub fn stop_cmd(&self) {
        debug!("AHCI PORT: STOP CMD");

        let cmd = self.command_status();

        cmd.set_st(false);
        cmd.set_fre(false);

        while cmd.get_fr() | cmd.get_cr() {}
    }

    pub fn configure(&self) {
        self.stop_cmd();
        debug!("AHCI PORT: CONFIGURING");

        debug!("Allocting command and FIS lists.");

        let cmd_list_byte_len = core::mem::size_of::<super::Command>() * 32;
        let cmd_list_ptr = unsafe {
            libsys::memory::malloc::get()
                .alloc(cmd_list_byte_len, core::num::NonZeroUsize::new(128))
                .unwrap()
                .into_parts()
                .0
        };
        debug!(
            "\tCommand list base address: {:?}:{}",
            cmd_list_ptr, cmd_list_byte_len
        );

        let fis_byte_len = 1024;
        let fis_base = unsafe {
            libsys::memory::malloc::get()
                .alloc(cmd_list_byte_len, core::num::NonZeroUsize::new(128))
                .unwrap()
                .into_parts()
                .0
        };
        debug!("\tFIS base address: {:fis_base?}:{fis_byte_len}");

        unsafe {
            debug!("Clearing command and FIS lists, and updating port metadata.");

            core::ptr::write_bytes(cmd_list_ptr, 0, cmd_list_byte_len);
            core::ptr::write_bytes(fis_base, 0, fis_byte_len);

            self.cmd_list.set_ptr(cmd_list_ptr as *mut _);
            self.fis_addr.set_ptr(fis_base as *mut _);
        }

        debug!("AHCI PORT: CONFIGURED");
        self.start_cmd();
    }

    pub fn read(&self, sector_base: usize, sector_count: u16) -> alloc::vec::Vec<u8> {
        use crate::drivers::ahci::{ATA_DEV_BUSY, ATA_DEV_DRQ, hba::fis};

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

        debug!("AHCI PORT: READ: CFG COMMAND FIS (DIS_REG_H2D)");
        let mut fis = super::fis::Hw2Dev::read(sector_base, sector_count);

        debug!("AHCI PORT: READ: CFG COMMAND");
        let command = &mut self.command_list()[0];
        command.reset(1, fis);
        command.set_write(false);

        debug!("AHCI PORT: READ: CFG PRDT ENTRY");
        let prdt_entry = &mut command.prdt_entries()[0];
        let buffer = alloc::vec![0u8; (sector_count as usize) * 512];
        prdt_entry.set_db_addr(Address::from_ptr(buffer.as_ptr()));
        prdt_entry.set_sector_count(sector_count as u32);

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

    // pub fn write(&mut self, sector_base: usize, data: &[u8]) {}
}

impl libsys::memory::Volatile for Port {}
