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

lazy_static::lazy_static! {
    pub static ref BRIDGES: Bridges = Bridges(spin::Mutex::new(
        crate::acpi::rdsp::xsdt::XSDT
            .find_sub_table::<crate::acpi::rdsp::xsdt::mcfg::MCFG>()
            .expect("XSDT does not contain an MCFG table")
            .iter()
            .filter_map(|entry| configure_host_bridge(entry).ok())
            .collect(),
    ));
}
