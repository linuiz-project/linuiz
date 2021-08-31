#[repr(transparent)]
pub struct CommandStatus {
    bits: u32,
}

impl CommandStatus {
    libkernel::bitfield_getter!(bits, st, 0);
    // SUD - Check CAP.SSS is 1 or 0 for RW or RO

    libkernel::bitfield_getter_ro!(bits, pod, 2);
    pub fn set_pod(&mut self, set: bool) -> Result<(), ()> {
        if self.get_cpd() {
            use bit_field::BitField;

            self.bits.set_bit(2, set);

            Ok(())
        } else {
            Err(())
        }
    }

    libkernel::bitfield_getter!(bits, clo, 3);
    libkernel::bitfield_getter!(bits, fre, 4);
    libkernel::bitfield_getter_ro!(bits, u32, ccs, 8..13);
    libkernel::bitfield_getter_ro!(bits, mpss, 13);
    libkernel::bitfield_getter_ro!(bits, fr, 14);
    libkernel::bitfield_getter_ro!(bits, cr, 15);
    libkernel::bitfield_getter_ro!(bits, cps, 16);
    // PMA - check CAP.SPM = 1 or 0 for RW or RO
    libkernel::bitfield_getter_ro!(bits, hpcp, 18);
    libkernel::bitfield_getter_ro!(bits, mpsp, 19);
    libkernel::bitfield_getter_ro!(bits, cpd, 20);
    libkernel::bitfield_getter_ro!(bits, esp, 21);
    libkernel::bitfield_getter_ro!(bits, fbscp, 22);
    libkernel::bitfield_getter!(bits, apste, 22);
    libkernel::bitfield_getter!(bits, atapi, 24);
    libkernel::bitfield_getter!(bits, dlae, 25);
    // ALPE - Check CAP.SALP is 1 or 0 for RW or Reserved
    // ASP - Check CAP.SALP is 1 or 0 for RW or Reserved
    libkernel::bitfield_getter!(bits, u32, icc, 28..32);
}

impl core::fmt::Debug for CommandStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Command Status Register")
            .field("ST", &self.get_st())
            // .field("SUD", &self.sud())
            .field("CLO", &self.get_clo())
            .field("FRE", &self.get_fre())
            // .field("CCS", &self.ccs())
            .field("MPSS", &self.get_mpss())
            .field("FR", &self.get_fr())
            .field("CR", &self.get_cr())
            .field("CPS", &self.get_cps())
            // .field("PMA", &self.pma())
            .field("HPCP", &self.get_hpcp())
            .field("MPSP", &self.get_mpsp())
            .field("CPD", &self.get_cpd())
            .field("ESP", &self.get_esp())
            .field("APSTE", &self.get_apste())
            .field("ATAPI", &self.get_atapi())
            .field("DLAE", &self.get_dlae())
            // .field("ALPE", &self.alpe())
            // .field("ASP", &self.asp())
            // .field("ICC", &self.icc())
            .finish()
    }
}
