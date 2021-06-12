use super::{HostBusAdapterPort, HostBusAdapterPortIterator};

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
    ports: [HostBusAdapterPort; 32],
}

impl HostBustAdapterMemory {
    pub fn ports(&self) -> HostBusAdapterPortIterator {
        HostBusAdapterPortIterator::new(&self.ports, self.ports_implemented)
    }
}
