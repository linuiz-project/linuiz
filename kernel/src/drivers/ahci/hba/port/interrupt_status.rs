use libkernel::{bitfield_getter, bitfield_getter_ro};

#[repr(transparent)]
pub struct InterruptStatus {
    bits: u32,
}

impl InterruptStatus {
    bitfield_getter!(bits, dhrs, 0);
    bitfield_getter!(bits, pss, 1);
    bitfield_getter!(bits, dss, 2);
    bitfield_getter!(bits, sdbs, 3);
    bitfield_getter_ro!(bits, ufs, 4);
    bitfield_getter!(bits, dps, 5);
    bitfield_getter_ro!(bits, pcs, 6);
    bitfield_getter!(bits, dmps, 7);
    bitfield_getter_ro!(bits, prcs, 22);
    bitfield_getter!(bits, ipms, 23);
    bitfield_getter!(bits, ofs, 24);
    bitfield_getter!(bits, infs, 26);
    bitfield_getter!(bits, ifs, 27);
    bitfield_getter!(bits, hbds, 28);
    bitfield_getter!(bits, hbfs, 29);
    bitfield_getter!(bits, tfes, 30);
    bitfield_getter!(bits, cpds, 31);

    pub fn clear(&mut self) {
        self.bits = 0;
    }
}

impl core::fmt::Debug for InterruptStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Interrupt Status")
            .field("DHRS", &self.get_dhrs())
            .field("PSS", &self.get_pss())
            .field("DSS", &self.get_dss())
            .field("UFS", &self.get_ufs())
            .field("DPS", &self.get_dps())
            .field("PCS", &self.get_pcs())
            .field("DMPS", &self.get_dmps())
            .field("PRCS", &self.get_prcs())
            .field("IPMS", &self.get_ipms())
            .field("OFS", &self.get_ofs())
            .field("INFS", &self.get_infs())
            .field("IFS", &self.get_ifs())
            .field("HBDS", &self.get_hbds())
            .field("HBFS", &self.get_hbfs())
            .field("TFES", &self.get_tfes())
            .field("CPDS", &self.get_cpds())
            .finish()
    }
}
