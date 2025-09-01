use crate::mem::Permissions;
use bit_field::BitField;

pub const PT_FLAG_EXEC_BIT: usize = 0;
pub const PT_FLAG_WRITE_BIT: usize = 1;

pub fn segment_to_mapping_permissions(segment_flags: u32) -> Permissions {
    match (
        segment_flags.get_bit(PT_FLAG_WRITE_BIT),
        segment_flags.get_bit(PT_FLAG_EXEC_BIT),
    ) {
        (false, false) => Permissions::ReadOnly,
        (true, false) => Permissions::ReadWrite,
        (false, true) => Permissions::ReadExecute,
        (true, true) => unreachable!("ELF segment is WX"),
    }
}
