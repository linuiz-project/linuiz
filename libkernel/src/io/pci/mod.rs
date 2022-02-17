mod bus;
mod device;
mod host;

pub use bus::*;
pub use device::*;
pub use host::*;

use alloc::vec::Vec;
use spin::Mutex;

pub struct Bridges(Mutex<Vec<PCIeHostBridge>>);
unsafe impl Send for Bridges {}
unsafe impl Sync for Bridges {}
impl core::ops::Deref for Bridges {
    type Target = Mutex<Vec<PCIeHostBridge>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub fn get_host_bridges(
    page_manager: Option<&crate::memory::PageManager>,
) -> impl Iterator<Item = PCIeHostBridge> + '_ {
    crate::acpi::rdsp::xsdt::XSDT
        .find_sub_table::<crate::acpi::rdsp::xsdt::mcfg::MCFG>()
        .expect("XSDT does not contain an MCFG table")
        .iter()
        .filter_map(move |entry| configure_host_bridge(entry, page_manager).ok())
}
