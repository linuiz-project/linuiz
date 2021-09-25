pub mod command;
pub mod queue;

use bit_field::BitField;
use core::{borrow::Borrow, convert::TryFrom, fmt};
use libkernel::{
    addr_ty::Physical,
    io::pci::{standard::StandardRegister, PCIeDevice, Standard},
    memory::volatile::{Volatile, VolatileCell},
    volatile_bitfield_getter, volatile_bitfield_getter_ro, Address, ReadOnly, ReadWrite,
};
use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, TryFromPrimitive)]
pub enum CPS {
    NotReported = 0b00,
    ControllerScope = 0b01,
    DomainScope = 0b10,
    NVMSubsystemScope = 0b11,
}

#[repr(transparent)]
pub struct Capabilities {
    value: VolatileCell<u64, ReadOnly>,
}

/// NVME Capabilities Register
/// An explanation of these values can be found at:
///     https://nvmexpress.org/wp-content/uploads/NVMe-NVM-Express-2.0a-2021.07.26-Ratified.pdf
///     Figure 36
impl Capabilities {
    volatile_bitfield_getter_ro!(value, u64, mqes, 0..16);
    volatile_bitfield_getter_ro!(value, cqr, 16);
    volatile_bitfield_getter_ro!(value, u64, ams, 17..19);
    // 19..24 reserved
    volatile_bitfield_getter_ro!(value, u64, to, 24..32);
    volatile_bitfield_getter_ro!(value, u64, dstrd, 32..36);
    volatile_bitfield_getter_ro!(value, nssrs, 36);
    volatile_bitfield_getter_ro!(value, u64, css, 37..45);
    volatile_bitfield_getter_ro!(value, bps, 45);

    pub fn get_cps(&self) -> CPS {
        CPS::try_from(self.value.read().get_bits(46..48)).unwrap()
    }

    volatile_bitfield_getter_ro!(value, u64, mpsmin, 48..52);
    volatile_bitfield_getter_ro!(value, u64, mpsmax, 52..56);
    volatile_bitfield_getter_ro!(value, pmrs, 56);
    volatile_bitfield_getter_ro!(value, cmbs, 57);
    volatile_bitfield_getter_ro!(value, nsss, 58);
    volatile_bitfield_getter_ro!(value, crwms, 59);
    volatile_bitfield_getter_ro!(value, crims, 60);
    // 60..64 reserved
}

impl Volatile for Capabilities {}

impl fmt::Debug for Capabilities {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NVME Capabilities")
            .field("MQES", &self.get_mqes())
            .field("CQR", &self.get_cqr())
            .field("AMS", &self.get_ams())
            .field("TO", &self.get_to())
            .field("DSTRD", &self.get_dstrd())
            .field("NSSRS", &self.get_nssrs())
            .field("CSS", &self.get_css())
            .field("BPS", &self.get_bps())
            .field("CPS", &self.get_cps())
            .field("MPSMIN", &self.get_mpsmin())
            .field("MPSMAX", &self.get_mpsmax())
            .field("PMRS", &self.get_pmrs())
            .field("NSSS", &self.get_nsss())
            .field("CRWMS", &self.get_crwms())
            .field("CRIMS", &self.get_crims())
            .finish()
    }
}

#[repr(transparent)]
pub struct Version(u32);

impl Version {
    pub fn major(&self) -> u16 {
        self.0.get_bits(16..32) as u16
    }

    pub fn minor(&self) -> u8 {
        self.0.get_bits(8..16) as u8
    }

    pub fn tertiary(&self) -> u8 {
        self.0.get_bits(0..8) as u8
    }
}

impl fmt::Debug for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Version")
            .field(&self.major())
            .field(&self.minor())
            .field(&self.tertiary())
            .finish()
    }
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum IOCommandSet {
    NVM = 0b000,
    FullIO = 0b110,
    Admin = 0b111,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum ArbitrationMechanism {
    RoundRobin = 0b000,
    WeightedRoundRobinWithUrgenPriorityClass = 0b001,
    VendorSpecific = 0b111,
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum ShutdownNotification {
    None = 0b00,
    Normal = 0b01,
    Abrupt = 0b10,
}

#[repr(transparent)]
pub struct ControllerConfiguration {
    value: VolatileCell<u32, ReadWrite>,
}

impl Volatile for ControllerConfiguration {}

impl ControllerConfiguration {
    volatile_bitfield_getter!(value, en, 0);

    pub fn get_css(&self) -> IOCommandSet {
        IOCommandSet::try_from(self.value.read().get_bits(4..7)).expect("CSS is reserved value")
    }

    pub fn set_css(&self, command_set: IOCommandSet) {
        self.value
            .write(*self.value.read().set_bits(4..7, command_set as u32))
    }

    pub fn get_mps(&self) -> u32 {
        self.value.read().get_bits(7..11)
    }

    pub fn set_mps(&self, mps: u32) {
        assert!(
            mps < 0b10000,
            "Provided memory page size must be no more than 4 bits."
        );
        assert!(
            !self.get_en(),
            "Memory page size may only be set when controller is not enabled."
        );

        self.value.write(*self.value.read().set_bits(7..11, mps));
    }

    pub fn get_ams(&self) -> ArbitrationMechanism {
        ArbitrationMechanism::try_from(self.value.read().get_bits(11..14))
            .expect("AMS is reserved value")
    }

    pub fn set_ams(&self, ams: ArbitrationMechanism) {
        self.value
            .write(*self.value.read().set_bits(11..14, ams as u32))
    }

    pub fn get_shn(&self) -> ShutdownNotification {
        ShutdownNotification::try_from(self.value.read().get_bits(14..16))
            .expect("SHN is resrved value")
    }

    pub fn set_shn(&self, shn: ShutdownNotification) {
        self.value
            .write(*self.value.read().set_bits(14..16, shn as u32))
    }

    pub fn get_iosqes(&self) -> u32 {
        self.value.read().get_bits(16..20)
    }

    pub fn get_iocqes(&self) -> u32 {
        self.value.read().get_bits(20..24)
    }

    // TODO CC.CRIME
}

impl fmt::Debug for ControllerConfiguration {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Controller Configuration")
            .field("Enabled", &self.get_en())
            .field("IO Command Set", &self.get_css())
            .field("Memory Page Size", &self.get_mps())
            .field("Arbitration Mechanism", &self.get_ams())
            .field("Shutdown Notification", &self.get_shn())
            .field("I/O Submission Queue Entry Size", &self.get_iosqes())
            .field("I/O Completion Queue Entry Size", &self.get_iocqes())
            .finish()
    }
}

#[repr(u32)]
#[derive(Debug, TryFromPrimitive)]
pub enum ShutdownStatus {
    Normal = 0b00,
    Occurring = 0b01,
    Complete = 0b10,
}

#[repr(transparent)]
pub struct ControllerStatus {
    value: VolatileCell<u32, ReadOnly>,
}

impl Volatile for ControllerStatus {}

impl ControllerStatus {
    volatile_bitfield_getter_ro!(value, rdy, 0);
    volatile_bitfield_getter_ro!(value, cfs, 1);

    pub fn get_shst(&self) -> ShutdownStatus {
        ShutdownStatus::try_from(self.value.read().get_bits(2..4)).expect("SHST is reserved value")
    }

    volatile_bitfield_getter_ro!(value, nssro, 4);
    volatile_bitfield_getter_ro!(value, pp, 5);
    volatile_bitfield_getter_ro!(value, st, 6);
}

impl fmt::Debug for ControllerStatus {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Controller Status")
            .field("Ready", &self.get_rdy())
            .field("Controller Fatal Status", &self.get_cfs())
            .field("Shutdown Status", &self.get_shst())
            .field("NVM Subsystem Reset Occurred", &self.get_nssro())
            .field("Processing Paused", &self.get_pp())
            .field("Shutdown Type", &self.get_st())
            .finish()
    }
}

pub struct Controller<'dev> {
    device: &'dev PCIeDevice<Standard>,
    pub admin_sub: queue::SubmissionQueue<'dev>,
    pub admin_com: queue::CompletionQueue<'dev>,
}

impl<'dev> Controller<'dev> {
    const CAP: usize = 0x0;
    const VER: usize = 0x8;
    const CC: usize = 0x14;
    const CSTS: usize = 0x18;
    const AQA: usize = 0x24;
    const ASQ: usize = 0x28;
    const ACQ: usize = 0x30;

    fn doorbell_offset(&self, queue_id: usize, is_start: bool, is_completion: bool) -> usize {
        let base_offset = 0x1000;
        let queue_offset = 2 * (queue_id + (is_completion as usize));
        let doorbell_stride = 4 << self.capabilities().get_dstrd();

        base_offset + (queue_offset * doorbell_stride)
    }

    pub fn from_device(device: &'dev PCIeDevice<Standard>) -> Self {
        let reg0 = device.get_register(StandardRegister::Register0).unwrap();
        let capabilities = unsafe { reg0.borrow::<Capabilities>(0).unwrap() };
        let (sub_entry_cnt, com_entry_cnt) = {
            let attribs = unsafe { reg0.read::<u32>(Self::AQA) }.unwrap();

            (
                (attribs.get_bits(0..12) as u16) + 1,
                (attribs.get_bits(16..28) as u16) + 1,
            )
        };

        let admin_sub = unsafe {
            queue::SubmissionQueue::from_addr(
                reg0.mapped_addr() + 0x1000,
                reg0.read::<Address<Physical>>(Self::ASQ).unwrap(),
                sub_entry_cnt,
            )
        };

        let admin_com = unsafe {
            queue::CompletionQueue::from_addr(
                reg0.mapped_addr() + 0x1004,
                reg0.read::<Address<Physical>>(Self::ACQ).unwrap(),
                com_entry_cnt,
            )
        };

        Self {
            device,
            admin_sub,
            admin_com,
        }
    }

    pub fn capabilities(&self) -> &Capabilities {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .borrow(Self::CAP)
                .unwrap()
        }
    }

    pub fn version(&self) -> Version {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read(Self::VER)
                .unwrap()
        }
    }

    pub fn controller_configuration(&self) -> &ControllerConfiguration {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .borrow(Self::CC)
                .unwrap()
        }
    }

    pub fn controller_status(&self) -> &ControllerStatus {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .borrow(Self::CSTS)
                .unwrap()
        }
    }

    /// The admin submission & completion queue sizes (in entries).
    ///     - 1st `u16`: submission queue
    ///     - 2nd `u16`: completion queue
    pub fn admin_queue_attribs(&self) -> (u16, u16) {
        let admin_queue_attribs = unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read::<u32>(Self::AQA)
        }
        .unwrap();

        let submission_size = (admin_queue_attribs.get_bits(0..12) as u16) + 1;
        let completion_size = (admin_queue_attribs.get_bits(16..28) as u16) + 1;

        assert!(
            (2..=4096).contains(&submission_size),
            "Maximum admin submission queue size is 2..=4096 (is {}).",
            submission_size
        );
        assert!(
            (2..=4096).contains(&completion_size),
            "Maximum admin completion queue size is 2..=4096 (is {}).",
            completion_size
        );

        (submission_size, completion_size)
    }

    pub fn get_admin_submission_queue_addr(&self) -> Address<Physical> {
        let queue_phys_addr = unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read::<Address<Physical>>(Self::ASQ)
        }
        .unwrap();

        assert!(
            queue_phys_addr.is_frame_aligned(),
            "Admin submission queue address is not frame aligned."
        );

        queue_phys_addr
    }

    pub fn get_admin_completion_queue_addr(&self) -> Address<Physical> {
        let queue_phys_addr = unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read::<Address<Physical>>(Self::ACQ)
        }
        .unwrap();

        assert!(
            queue_phys_addr.is_frame_aligned(),
            "Admin completion queue address is not frame aligned."
        );

        queue_phys_addr
    }
}

impl fmt::Debug for Controller<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NVMe Device")
            .field("Capabilities", &self.capabilities())
            .field("Version", &self.version())
            .field("Controller Configuration", &self.controller_configuration())
            .field("Controller Status", &self.controller_status())
            .field("Admin Queue Attributes", &self.admin_queue_attribs())
            .field(
                "Admin Submission Queue Address",
                &self.get_admin_submission_queue_addr(),
            )
            .field(
                "Admin Completion Queue Address",
                &self.get_admin_completion_queue_addr(),
            )
            .finish()
    }
}
