pub mod port;

use port::HostBusAdapterPort;

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
    #[inline(always)]
    const fn ports_implemented(&self) -> usize {
        let mut bits = 0;
        let mut bit = 1;

        while (self.ports_implemented & bit) > 0 {
            bits += 1;
            bit <<= 1;
        }

        bits
    }

    pub fn ports(&self) -> &[HostBusAdapterPort] {
        let len = self.ports_implemented();
        &self.ports[0..len]
    }

    pub fn ports_mut(&mut self) -> &mut [HostBusAdapterPort] {
        let len = self.ports_implemented();
        &mut self.ports[0..len]
    }
}


