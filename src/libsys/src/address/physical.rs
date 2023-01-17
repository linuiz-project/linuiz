pub struct Physical;

impl super::Addressable for Physical {
    type InitType = usize;
    type ReprType = usize;

    fn new(init: Self::InitType) -> Option<Self::ReprType> {
        crate::constants::checked_phys_canonical(init).then_some(init)
    }

    fn new_truncate(init: Self::InitType) -> Self::ReprType {
        init & !crate::constants::PHYS_NON_CANONICAL_MASK
    }
}
