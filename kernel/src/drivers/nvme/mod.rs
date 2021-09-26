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
pub enum ControllerPowerScope {
    NotReported = 0b00,
    ControllerScope = 0b01,
    DomainScope = 0b10,
    NVMSubsystemScope = 0b11,
}

bitflags::bitflags! {
    pub struct CommandSetsSupported: u8 {
        const NVM = 1 << 0;
        const IO = 1 << 6;
        const ONLY_ADMIN = 1 << 7;
    }
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

    pub fn get_css(&self) -> CommandSetsSupported {
        CommandSetsSupported::from_bits_truncate(self.value.read().get_bits(37..45) as u8)
    }

    volatile_bitfield_getter_ro!(value, bps, 45);

    pub fn get_cps(&self) -> ControllerPowerScope {
        ControllerPowerScope::try_from(self.value.read().get_bits(46..48)).unwrap()
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
pub enum CommandSet {
    NVM = 0b000,
    IO = 0b110,
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

    pub fn get_css(&self) -> CommandSet {
        CommandSet::try_from(self.value.read().get_bits(4..7)).expect("CSS is reserved value")
    }

    pub fn set_css(&self, command_set: CommandSet) {
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

    pub fn set_iosqes(&self, iosqes: u32) {
        self.value
            .write(*self.value.read().set_bits(16..20, iosqes))
    }

    pub fn get_iocqes(&self) -> u32 {
        self.value.read().get_bits(20..24)
    }

    pub fn set_iocqes(&self, iocqes: u32) {
        self.value
            .write(*self.value.read().set_bits(20..24, iocqes))
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

#[repr(C)]
pub struct InterruptMask {
    set: VolatileCell<u32, ReadWrite>,
    clear: VolatileCell<u32, ReadWrite>,
}

impl InterruptMask {
    pub fn mask_vector(&self, index: usize) {
        assert!(index < 32, "Index must be 0..32.");

        self.set.write(*self.set.read().set_bit(index, true));
    }

    pub fn unmask_vector(&self, index: usize) {
        assert!(index < 32, "Index must be 0..32.");

        self.clear.write(*self.clear.read().set_bit(index, true));
    }

    pub fn raw_bits_str(&self) -> alloc::string::String {
        alloc::format!("{:b}", self.set.read())
    }
}

pub struct Controller<'dev> {
    device: &'dev PCIeDevice<Standard>,
    pub admin_sub: Option<queue::SubmissionQueue<'dev>>,
    pub admin_com: Option<queue::CompletionQueue<'dev>>,
}

impl<'dev> Controller<'dev> {
    const CAP: usize = 0x0;
    const VER: usize = 0x8;
    const INTMS: usize = 0xC;
    const INTMC: usize = 0x10;
    const CC: usize = 0x14;
    const CSTS: usize = 0x1C;
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
        let nvme = Self {
            device,
            admin_sub: None,
            admin_com: None,
        };

        let cc = nvme.controller_configuration();
        debug!("Resetting controller (disable & wait for unready).");
        unsafe { nvme.safe_set_enable(false) };

        debug!("Initializing controller to base values.");
        cc.set_iosqes(6);
        cc.set_iocqes(4);
        cc.set_shn(ShutdownNotification::None);
        cc.set_ams(ArbitrationMechanism::RoundRobin);
        cc.set_mps(0); // 4KiB pages
        let css = nvme.capabilities().get_css();
        cc.set_css(if css.contains(CommandSetsSupported::ONLY_ADMIN) {
            CommandSet::Admin
        } else if css.contains(CommandSetsSupported::IO) {
            CommandSet::IO
        } else if css.contains(CommandSetsSupported::NVM) {
            CommandSet::NVM
        } else {
            panic!("NVMe driver reports invalid CAP.CSS value: {:?}", css)
        });

        nvme
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

    pub fn interrupt_mask(&self) -> &InterruptMask {
        unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read(Self::INTMS)
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
    ///     - 1st `u16`: submission queue length
    ///     - 2nd `u16`: completion queue length
    pub fn get_admin_queue_sizes(&self) -> (u16, u16) {
        let admin_queue_attribs = unsafe {
            self.device
                .get_register(StandardRegister::Register0)
                .unwrap()
                .read::<u32>(Self::AQA)
        }
        .unwrap();

        (
            (admin_queue_attribs.get_bits(0..12) as u16) + 1,
            (admin_queue_attribs.get_bits(16..28) as u16) + 1,
        )
    }

    pub fn set_admin_queue_sizes(&self, sq_size: u16, cq_size: u16) {
        assert!(
            !self.controller_configuration().get_en(),
            "Cannot configure controller when enabled."
        );

        assert!(
            sq_size < 1 << 12,
            "Maximum submission queue index is 12 bits."
        );
        assert!(
            cq_size < 1 << 12,
            "Maximum completion queue index is 12 bits."
        );

        unsafe {
            let reg0 = self
                .device
                .get_register(StandardRegister::Register0)
                .unwrap();

            let mut attribs = reg0.read::<u32>(Self::AQA).unwrap();
            attribs.set_bits(0..12, sq_size as u32);
            attribs.set_bits(16..28, cq_size as u32);
            reg0.write(Self::AQA, attribs).unwrap();
        }
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

    pub unsafe fn safe_set_enable(&self, enable: bool) -> bool {
        self.controller_configuration().set_en(enable);

        let csts = self.controller_status();
        let max_wait = self.capabilities().get_to() * 500;
        let mut msec_waited = 0;
        while !csts.get_cfs() && csts.get_rdy() != enable && msec_waited < max_wait {
            crate::timer::sleep_msec(10);
            msec_waited += 10;
        }

        !csts.get_cfs() && csts.get_rdy() == enable
    }

    pub fn configure_admin_submission_queue(&mut self, entry_count: u16) {
        assert!(
            !self.controller_configuration().get_en(),
            "Cannot configure controller when enabled."
        );
        unsafe {
            let reg0 = self
                .device
                .get_register(StandardRegister::Register0)
                .unwrap();

            // Configure queue size in hardware.
            reg0.write(
                Self::AQA,
                *reg0
                    .read::<u32>(Self::AQA)
                    .unwrap()
                    .set_bits(0..12, (entry_count - 1) as u32),
            )
            .unwrap();

            // Configure queue address & queue abstraction.
            let sub_queue = queue::SubmissionQueue::new(
                reg0.mapped_addr() + 0x1000,
                entry_count,
                (self.controller_configuration().get_iosqes() << 4) as usize,
            );
            reg0.write(Self::ASQ, sub_queue.base_phys_addr().as_usize() as u64)
                .unwrap();
            self.admin_sub = Some(sub_queue);
        }
    }

    pub fn configure_admin_completion_queue(&mut self, entry_count: u16) {
        assert!(
            !self.controller_configuration().get_en(),
            "Cannot configure controller when enabled."
        );

        unsafe {
            let reg0 = self
                .device
                .get_register(StandardRegister::Register0)
                .unwrap();

            // Configure queue size in hardware.
            reg0.write(
                Self::AQA,
                *reg0
                    .read::<u32>(Self::AQA)
                    .unwrap()
                    .set_bits(16..28, (entry_count - 1) as u32),
            )
            .unwrap();

            // Configure queue address & queue abstraction.
            let com_queue = queue::CompletionQueue::new(
                reg0.mapped_addr() + 0x1000,
                entry_count,
                (self.controller_configuration().get_iocqes() << 4) as usize,
            );
            reg0.write(Self::ACQ, com_queue.base_phys_addr().as_usize() as u64)
                .unwrap();
            self.admin_com = Some(com_queue);
        }
    }
}

impl fmt::Debug for Controller<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NVMe Device")
            .field("Capabilities", &self.capabilities())
            .field("Version", &self.version())
            .field("Interrupt Mask", &self.interrupt_mask().raw_bits_str())
            .field("Controller Configuration", &self.controller_configuration())
            .field("Controller Status", &self.controller_status())
            .field("Admin Queue Attributes", &self.get_admin_queue_sizes())
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
