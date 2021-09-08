use bit_field::BitField;
use core::convert::TryFrom;
use libkernel::{volatile::VolatileCell, volatile_bitfield_getter_ro};
use num_enum::TryFromPrimitive;

#[repr(u64)]
#[derive(Debug, TryFromPrimitive)]
pub enum NVMECPS {
    NotReported = 0b00,
    ControllerScope = 0b01,
    DomainScope = 0b10,
    NVMSubsystemScope = 0b11,
}

#[repr(transparent)]
pub struct NVMECapabilities {
    value: VolatileCell<u64>,
}

/// NVME Capabilities Register
/// An explanation of these values can be found at:
///     https://nvmexpress.org/wp-content/uploads/NVMe-NVM-Express-2.0a-2021.07.26-Ratified.pdf
///     Figure 36
impl NVMECapabilities {
    volatile_bitfield_getter_ro!(value, u64, mqes, 0..16);
    volatile_bitfield_getter_ro!(value, cqr, 16);
    volatile_bitfield_getter_ro!(value, u64, ams, 17..19);
    // 19..24 reserved
    volatile_bitfield_getter_ro!(value, u64, to, 24..32);
    volatile_bitfield_getter_ro!(value, u64, dstrd, 32..36);
    volatile_bitfield_getter_ro!(value, nssrs, 36);
    volatile_bitfield_getter_ro!(value, u64, css, 37..45);
    volatile_bitfield_getter_ro!(value, bps, 45);

    pub fn cps(&self) -> NVMECPS {
        NVMECPS::try_from(self.value.read().get_bits(46..48)).unwrap()
    }

    volatile_bitfield_getter_ro!(value, u64, mpsmin, 48..52);
    volatile_bitfield_getter_ro!(value, u64, mpsmax, 52..56);
    volatile_bitfield_getter_ro!(value, u64, pmrs, 56);
    volatile_bitfield_getter_ro!(value, u64, cmbs, 57);
    volatile_bitfield_getter_ro!(value, u64, nsss, 58);
    volatile_bitfield_getter_ro!(value, u64, crwms, 59);
    volatile_bitfield_getter_ro!(value, u64, crims, 60);
    // 60..64 reserved
}