use alloc::boxed::Box;
use bit_field::BitField;
use core::{
    fmt::{Debug, Formatter, Result},
    marker::PhantomData,
};
use lib::{addr_ty::Physical, Address};
use num_enum::TryFromPrimitive;

// #[repr(u8)]
// #[derive(Debug, TryFromPrimitive)]
// pub enum Opcode {
//     DeleteIOSubmissionQueue = 0x0,
//     CreateIOSubmissionQueue = 0x1,
//     GetLogPage = 0x2,
//     DeleteIOCompletionQueue = 0x4,
//     CreateIOCompletionQueue = 0x5,
//     Identify = 0x6,
//     Abort = 0x8,
//     SetFeatures = 0x9,
//     GetFeatures = 0xA,
//     AsyncEventRequest = 0xC,
//     // TODO
// }

pub enum AdminCommand {
    Identify { ctrl_id: u16 },
}

impl AdminCommand {
    pub const fn get_opcode(&self) -> u8 {
        match self {
            AdminCommand::Identify { ctrl_id } => 0x6,
        }
    }
}

// impl super::Command<Admin> {
//     pub const fn create_io_completion_queue(
//         id: u16,
//         len: u16,
//         queue_ptr: super::DataPointer,
//         phys_contiguous: bool,
//         int_vector: Option<u16>,
//     ) -> Self {
//         Self {
//             opcode: Opcode::CreateIOCompletionQueue as u8,
//             fuse_psdt: ((super::PSDT::PRP as u8) << 6) | (super::FuseOperation::Normal as u8),
//             command_id: 0, // TODO support this
//             ns_id: 0,
//             cdw2: 0,
//             cdw3: 0,
//             mdata_ptr: Address::zero(),
//             data_ptr: queue_ptr,
//             cdw10: ((len << 16) as u32) | (id as u32),
//             cdw11: match int_vector {
//                 Some(vector) => ((vector as u32) << 16) | (1 << 1) | (phys_contiguous as u32),
//                 None => phys_contiguous as u32,
//             },
//             cdw12: 0,
//             cdw13: 0,
//             cdw14: 0,
//             cdw15: 0,
//             marker: PhantomData,
//         }
//     }
// }

#[repr(C)]
pub struct Identify {
    vendor_id: u16,
    subsys_vendor_id: u16,
    serial_number: [u8; 20],
    model_number: [u8; 40],
    firmware_rev: [u8; 8],
    rec_arb_burst: u8,
    ieee: [u8; 3],
    cmic: u8,
    mdts: u8,
    controller_id: u16,
    version: [u8; 3],
    rsvd1: [u8; 4096 - 83],
}

impl Debug for Identify {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> Result {
        formatter
            .debug_struct("NVMe Controller Identify")
            .field("Vendor ID", &format_args!("0x{:X}", self.vendor_id))
            .field(
                "Subsystem Vendor ID",
                &format_args!("0x{:X}", self.subsys_vendor_id),
            )
            .field(
                "Serial Number",
                &core::str::from_utf8(&self.serial_number).unwrap(),
            )
            .field(
                "Model Number",
                &core::str::from_utf8(&self.model_number).unwrap(),
            )
            .field(
                "Firmware Revision",
                &core::str::from_utf8(&self.firmware_rev).unwrap(),
            )
            .field(
                "Recommended Arbitration Burst",
                &format_args!("2^{}", self.rec_arb_burst),
            )
            .field("IEEE OUI Identifier", &self.ieee)
            .field("Maybe Multiple Subsystem Port", &self.cmic.get_bit(0))
            .field("Maybe Multiple Controllers", &self.cmic.get_bit(1))
            .field("SR-IOV Virtual Function Association", &self.cmic.get_bit(2))
            .field("Asymmetric Namespace Access", &self.cmic.get_bit(3))
            .field(
                "Maximum Data Transfer Size",
                &format_args!(
                    "{:?}",
                    match self.mdts {
                        0 => None,
                        mdts => Some(2_u32.pow(self.mdts as u32)),
                    }
                ),
            )
            .field(
                "Version",
                &format_args!(
                    "{}.{}.{}",
                    self.version[0], self.version[1], self.version[2]
                ),
            )
            .finish()
    }
}
