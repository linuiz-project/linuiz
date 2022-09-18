pub mod command;
pub mod queue;

use alloc::{boxed::Box, collections::BTreeMap};
use bit_field::BitField;
use core::{convert::TryFrom, fmt, marker::PhantomData, mem::MaybeUninit, sync::atomic::AtomicU16};
use libcommon::{
    io::pci::{standard::StandardRegister, PCIeDevice, Standard},
    memory::{
        page_aligned_allocator,
        volatile::{Volatile, VolatileCell},
        PageAlignedBox,
    },
    sync::{SuccessSource, SuccessToken, ValuedSuccessToken},
    volatile_bitfield_getter, volatile_bitfield_getter_ro, Address, Physical, ReadOnly, ReadWrite,
};
use num_enum::TryFromPrimitive;
use spin::{Mutex, MutexGuard};

#[repr(u64)]
#[derive(Debug, TryFromPrimitive)]
pub enum ControllerPowerScope {
    NotReported = 0b00,
    ControllerScope = 0b01,
    DomainScope = 0b10,
    NVMSubsystemScope = 0b11,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct CommandSetsSupported: u8 {
        const NVM = 1 << 0;
        const IO = 1 << 6;
        const ADMIN = 1 << 7;
    }
}

impl CommandSetsSupported {
    pub fn into_command_set(self) -> CommandSet {
        if self.contains(Self::ADMIN) {
            CommandSet::Admin
        } else if self.contains(Self::IO) {
            CommandSet::IO
        } else if self.contains(Self::NVM) {
            CommandSet::NVM
        } else {
            panic!("Invalid state for CAP.CSS: {:?}", self)
        }
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
pub struct Version(VolatileCell<u32, ReadOnly>);

impl Version {
    pub fn major(&self) -> u16 {
        self.0.read().get_bits(16..32) as u16
    }

    pub fn minor(&self) -> u8 {
        self.0.read().get_bits(8..16) as u8
    }

    pub fn tertiary(&self) -> u8 {
        self.0.read().get_bits(0..8) as u8
    }
}

impl Volatile for Version {}

impl fmt::Debug for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_tuple("Version").field(&self.major()).field(&self.minor()).field(&self.tertiary()).finish()
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
        self.value.write(*self.value.read().set_bits(4..7, command_set as u32))
    }

    pub fn get_mps(&self) -> u32 {
        self.value.read().get_bits(7..11)
    }

    pub fn set_mps(&self, mps: u32) {
        assert!(mps < 0b10000, "Provided memory page size must be no more than 4 bits.");
        assert!(!self.get_en(), "Memory page size may only be set when controller is not enabled.");
        self.value.write(*self.value.read().set_bits(7..11, mps));
    }

    pub fn get_ams(&self) -> ArbitrationMechanism {
        ArbitrationMechanism::try_from(self.value.read().get_bits(11..14)).expect("AMS is reserved value")
    }

    pub fn set_ams(&self, ams: ArbitrationMechanism) {
        self.value.write(*self.value.read().set_bits(11..14, ams as u32))
    }

    pub fn get_shn(&self) -> ShutdownNotification {
        ShutdownNotification::try_from(self.value.read().get_bits(14..16)).expect("SHN is resrved value")
    }

    pub fn set_shn(&self, shn: ShutdownNotification) {
        self.value.write(*self.value.read().set_bits(14..16, shn as u32))
    }

    pub fn get_iosqes(&self) -> u32 {
        self.value.read().get_bits(16..20)
    }

    pub fn set_iosqes(&self, iosqes: u32) {
        self.value.write(*self.value.read().set_bits(16..20, iosqes))
    }

    pub fn get_iocqes(&self) -> u32 {
        self.value.read().get_bits(20..24)
    }

    pub fn set_iocqes(&self, iocqes: u32) {
        self.value.write(*self.value.read().set_bits(20..24, iocqes))
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
            .field("Fatal Status", &self.get_cfs())
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

impl Volatile for InterruptMask {}

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

#[derive(Debug)]
pub enum ControllerEnableError {
    FatalStatus,
    NoReady,
}

pub struct Controller<'dev> {
    device: &'dev PCIeDevice<Standard>,
    msix: libcommon::io::pci::standard::MSIX<'dev>,
    next_sub_queue_id: AtomicU16,
    next_com_queue_id: AtomicU16,
    admin_sub: Mutex<queue::Queue<'dev, queue::Submission>>,
    admin_com: Mutex<queue::Queue<'dev, queue::Completion>>,
    pending_cmds: Mutex<BTreeMap<u16, SuccessSource>>,
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

    pub fn from_device_and_configure(
        device: &'dev PCIeDevice<Standard>,
        sub_entry_count: u16,
        com_entry_count: u16,
    ) -> Self {
        let nvme = {
            let reg0 = device.get_register(StandardRegister::Register0).unwrap();

            let admin_sub = queue::Queue::<queue::Submission>::new(reg0, 0, sub_entry_count);
            let admin_com = queue::Queue::<queue::Completion>::new(reg0, 0, com_entry_count);
            reg0.write(Self::ASQ, admin_sub.get_phys_addr().as_u64());
            reg0.write(Self::ACQ, admin_com.get_phys_addr().as_u64());
            reg0.write(Self::AQA, ((com_entry_count as u32) << 16) | (sub_entry_count as u32));

            Self {
                device,
                msix: device.find_msix().expect("MSI-X is required for NVMe controller creation."),
                next_sub_queue_id: AtomicU16::new(1),
                next_com_queue_id: AtomicU16::new(1),
                admin_sub: Mutex::new(admin_sub),
                admin_com: Mutex::new(admin_com),
                pending_cmds: Mutex::new(BTreeMap::new()),
            }
        };

        unsafe {
            nvme.set_enable_and_wait(false).expect("NVMe controller failed to reset");
        }
        debug!("NVMe controller successfully reset.");

        let cc = nvme.config();
        cc.set_css(nvme.capabilities().get_css().into_command_set());
        cc.set_ams(ArbitrationMechanism::RoundRobin);
        cc.set_mps(0); // 4KiB pages
        cc.set_iosqes(6); // 64 bytes (2^6)
        cc.set_iocqes(4); // 16 bytes (2^4)

        // Configure MSI-X for admin completion queue.
        // REMARK:  This needs to be before the enable, as QEMU tracks
        //          driver message IRQ usage internally, and doesn't
        //          'use' the first interrupt message if MSI-X isn't
        //          enabled when the controller starts.
        //
        //          I'm unsure what behaviour exists on real hardware.

        nvme.msix.set_enable(true);
        nvme.msix.set_function_mask(false);
        nvme.msix[0].configure(
            unsafe { crate::cpu::get_id() as u8 },
            // Specific vector should be dynamically selected
            // TODO possibly dynamically selected with special attributes per vector?
            //      i.e. separate interrupts for completions, DMA, etc.
            //      or a single interrupts per device? ***** this seems limiting
            crate::interrupts::Vector::Storage0 as u8,
            libcommon::InterruptDeliveryMode::Fixed,
        );
        nvme.msix[0].set_masked(false);

        unsafe {
            nvme.set_enable_and_wait(true).expect("NVMe driver failed to enable");
        }

        nvme
    }

    pub fn capabilities(&self) -> &Capabilities {
        unsafe { self.device.get_register(StandardRegister::Register0).unwrap().borrow(Self::CAP) }
    }

    pub fn version(&self) -> &Version {
        unsafe { self.device.get_register(StandardRegister::Register0).unwrap().borrow(Self::VER) }
    }

    pub fn interrupt_mask(&self) -> &InterruptMask {
        unsafe { self.device.get_register(StandardRegister::Register0).unwrap().borrow(Self::INTMS) }
    }

    pub fn config(&self) -> &ControllerConfiguration {
        unsafe { self.device.get_register(StandardRegister::Register0).unwrap().borrow(Self::CC) }
    }

    pub fn status(&self) -> &ControllerStatus {
        unsafe { self.device.get_register(StandardRegister::Register0).unwrap().borrow(Self::CSTS) }
    }

    pub unsafe fn set_enable_and_wait(&self, enabled: bool) -> Result<(), ControllerEnableError> {
        debug!("Resetting controller to enabled state: {enabled}.");
        self.config().set_en(enabled);
        let csts = self.status();
        let max_wait = self.capabilities().get_to() * 500;
        let mut msec_waited = 0;

        debug!("Waiting up to {}ms for controller to finalize enable state.", max_wait);
        while csts.get_rdy() != enabled && !csts.get_cfs() && msec_waited < max_wait {
            const SLEEP_INTERVAL: u64 = 100;

            crate::clock::busy_wait_msec(SLEEP_INTERVAL);
            msec_waited += SLEEP_INTERVAL;
        }

        if csts.get_cfs() {
            Err(ControllerEnableError::FatalStatus)
        } else if csts.get_rdy() != enabled {
            Err(ControllerEnableError::NoReady)
        } else {
            Ok(())
        }
    }

    fn next_command_id(pending_cmds: &MutexGuard<BTreeMap<u16, SuccessSource>>) -> u16 {
        // TODO optimize this
        let mut command_id = u16::MAX;
        for id in u16::MIN..u16::MAX {
            if !pending_cmds.contains_key(&id) {
                command_id = id;
                break;
            }
        }

        if command_id == u16::MAX {
            panic!("No more command IDs available.");
        } else {
            command_id
        }
    }

    pub fn submit_admin_command(&self, command: command::admin::AdminCommand) -> PendingCommand {
        let mut pending_cmds = self.pending_cmds.lock();
        let command_id = Self::next_command_id(&pending_cmds);

        use command::{
            admin::{AdminCommand, Identify},
            Command, DataPointer, FuseOperation, PSDT,
        };

        let opcode = command.get_opcode();
        match command {
            AdminCommand::Identify { ctrl_id } => {
                // Allocate the necessary memory for returning the command value.
                let memory = PageAlignedBox::<Identify>::new_uninit_in(page_aligned_allocator());
                let phys_addr = Address::<Physical>::new(
                    crate::memory::get_kernel_page_manager()
                        .get_mapped_to(&libcommon::memory::Page::from_ptr(memory.as_ptr()))
                        .unwrap(),
                );

                // Construct the command with the provided data.
                let command = Command {
                    opcode,
                    fuse_psdt: ((PSDT::PRP as u8) << 6) | (FuseOperation::Normal as u8),
                    command_id,
                    ns_id: 0,
                    cdw2: 0,
                    cdw3: 0,
                    mdata_ptr: Address::zero(),
                    data_ptr: DataPointer::new_prp(phys_addr, None),
                    cdw10: ((ctrl_id as u32) << 16) | 0b1, // TODO implement CNS
                    cdw11: 0,                              // Ensure CSI or CNS Specific Identifier are not required,
                    cdw12: 0,
                    cdw13: 0,
                    cdw14: 0, // Ensure no UUID is required, or possibly allow providing one (?)
                    cdw15: 0,
                };

                // Create the success synchronization.
                let (success_source, success_token) = SuccessSource::new_valued(unsafe { memory });

                // Pend command success synchronization, and submit.
                pending_cmds.insert(command_id, success_source);
                self.admin_sub.lock().submit_command(command);

                PendingCommand::Identify(success_token)
            }
        }
    }

    // TODO submit_command to use lpu::processor_id to index the submission and completion queues

    pub fn run(&self) -> ! {
        loop {
            let mut admin_com = self.admin_com.lock();
            if let Some(cmd_result) = admin_com.next_cmd_result() {
                let mut pending_cmds = self.pending_cmds.lock();
                let success_source = pending_cmds
                    .remove(&cmd_result.get_command_id())
                    .expect("NVMe completion provided unknown command ID");

                use command::{GenericStatus, StatusCode};
                match cmd_result.get_status().status_code() {
                    StatusCode::Generic(GenericStatus::SuccessfulCompletion) => success_source.complete(true),
                    _ => success_source.complete(false),
                }
            }
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
            .field("MSIX", &self.msix)
            .field("Controller Configuration", &self.config())
            .field("Controller Status", &self.status())
            .field("Admin Submission Queue Address", &self.admin_sub)
            .field("Admin Completion Queue Address", &self.admin_com)
            .finish()
    }
}

pub enum PendingCommand {
    Identify(ValuedSuccessToken<PageAlignedBox<MaybeUninit<command::admin::Identify>>>),
    Generic(SuccessToken),
}

pub fn exec_driver() {
    use libcommon::io::pci;

    let nvme: Controller = crate::PCIE_DEVICES
        .iter()
        .find_map(|device_variant| match device_variant {
            pci::DeviceVariant::Standard(device)
                if device.class() == pci::DeviceClass::MassStorageController && device.subclass() == 0x08 =>
            {
                Some(Controller::from_device_and_configure(&device, 4, 4))
            }
            _ => None,
        })
        // TODO exit task syscall instead ?
        .expect("No NVMe device attached.");

    nvme.run()
}
