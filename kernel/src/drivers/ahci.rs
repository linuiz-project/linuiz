use libkernel::{
    io::pci::{BaseAddressRegisterVariant, PCIeDevice, Standard},
    memory::mmio::{Mapped, MMIO},
};
use spin::MutexGuard;

#[repr(C)]
#[derive(Debug)]
pub struct HostBusAdapterPort {
    /// Note: In the specificaiton, this is two 32-bit values
    command_list_base: u64,
    fis_base_address: u64,
    interrupt_status: u32,
    interrupt_enable: u32,
    command_status: u32,
    _reserved0: [u8; 0x4],
    task_file_data: u32,
    signature: u32,
    sata_status: u32,
    sata_control: u32,
    sata_error: u32,
    sata_active: u32,
    command_issue: u32,
    sata_notification: u32,
    fis_switch_control: u32,
    _reserved1: [u8; 0xB],
    _vendor0: [u8; 0x4],
}

#[repr(C)]
#[derive(Debug)]
pub struct HostBustAdapterMemory {
    host_capability: u32,
    global_host_control: u32,
    interrupt_status: u32,
    ports_implemented: u32,
    version: u32,
    ccc_control: u32,
    ccc_ports: u32,
    enclosure_management_location: u32,
    enclosure_management_control: u32,
    host_capabilities_extended: u32,
    bios_handoff_control_status: u32,
    _reserved0: [u8; 0x74],
    _vendor0: [u8; 0x60],
    port: HostBusAdapterPort,
}

pub struct AHCI<'dev> {
    device: &'dev PCIeDevice<Standard>,
    hba_memory: MutexGuard<'dev, MMIO<Mapped>>,
}

impl<'dev> AHCI<'dev> {
    pub fn from_pcie_device(device: &'dev PCIeDevice<Standard>) -> Self {
        trace!("Using PCIe device for AHCI driver:\n{:#?}", device);

        info!("{:?}", device.reg0());
        info!("{:?}", device.reg1());
        info!("{:?}", device.reg2());
        info!("{:?}", device.reg3());
        info!("{:?}", device.reg4());
        info!("{:?}", device.reg5());

        if let Some(reg_mmio) = device.reg5() {
            info!(
                "{:?}",
                libkernel::memory::falloc::get()
                    .iter()
                    .nth(reg_mmio.physical_addr().frame_index())
            );

            Self {
                device,
                hba_memory: reg_mmio,
            }
        } else {
            panic!("device's host bust adapter is an incorrect register type")
        }
    }

    pub fn hba_memory(&'dev self) -> &'dev HostBustAdapterMemory {
        unsafe { self.hba_memory.read(0).unwrap() }
    }
}
