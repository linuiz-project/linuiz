use libkernel::bit_switch::{BitSwitch32, ReadOnly, ReadWrite};

#[repr(transparent)]
pub struct CommandStatus(u32);

impl CommandStatus {
    pub fn st(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 0)
    }
    pub fn st_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 0)
    }

    pub fn sud(&mut self) -> BitSwitch32<ReadWrite> {
        todo!("Check CAP.SSS is 1");
        // BitSwitch32::<ReadWrite>::new(&mut self.0, 1)
    }

    pub fn pod(&mut self) -> Result<BitSwitch32<ReadWrite>, BitSwitch32<ReadOnly>> {
        if self.cpd().get() {
            Ok(BitSwitch32::<ReadWrite>::new(&mut self.0, 2))
        } else {
            Err(BitSwitch32::<ReadOnly>::new(&self.0, 2))
        }
    }
    pub fn pod_ro(&self) -> Result<BitSwitch32<ReadOnly>, BitSwitch32<ReadOnly>> {
        if self.cpd_ro().get() {
            Ok(BitSwitch32::<ReadOnly>::new(&self.0, 2))
        } else {
            Err(BitSwitch32::<ReadOnly>::new(&self.0, 2))
        }
    }

    pub fn clo(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 3)
    }
    pub fn clo_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 3)
    }

    pub fn fre(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 4)
    }
    pub fn fre_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 4)
    }

    pub fn mpss(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 13)
    }

    pub fn fr(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 14)
    }

    pub fn cr(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 15)
    }

    pub fn cps(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 16)
    }

    pub fn pma(&mut self) -> Result<BitSwitch32<ReadWrite>, BitSwitch32<ReadOnly>> {
        todo!("Check CAP.SPM is 1 for Ok, 0 for Err");
        // BitSwitch32::<ReadWrite>::new(&mut self.0, 1)
    }

    pub fn hpcp(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 18)
    }

    pub fn mpsp(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 19)
    }

    pub fn cpd(&mut self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&mut self.0, 20)
    }
    pub fn cpd_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 20)
    }

    pub fn esp(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 21)
    }

    pub fn fbscp(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 12)
    }

    pub fn apste(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 23)
    }
    pub fn apste_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 23)
    }

    pub fn atapi(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 24)
    }
    pub fn atapi_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 24)
    }

    pub fn dlae(&mut self) -> BitSwitch32<ReadWrite> {
        BitSwitch32::<ReadWrite>::new(&mut self.0, 25)
    }
    pub fn dlae_ro(&self) -> BitSwitch32<ReadOnly> {
        BitSwitch32::<ReadOnly>::new(&self.0, 25)
    }

    pub fn alpe(&mut self) -> Option<BitSwitch32<ReadWrite>> {
        todo!("Check CAP.SALP is 1 or None");
        // BitSwitch32::<ReadWrite>::new(&mut self.0, 1)
    }

    pub fn asp(&mut self) -> Option<BitSwitch32<ReadWrite>> {
        todo!("Check CAP.SALP is 1 or None");
        // BitSwitch32::<ReadWrite>::new(&mut self.0, 1)
    }
}

impl core::fmt::Debug for CommandStatus {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Command Status Register")
            .field("ST", &self.st_ro().get())
            // .field("SUD", &self.sud())
            .field("CLO", &self.clo_ro().get())
            .field("FRE", &self.fre_ro().get())
            // .field("CCS", &self.ccs())
            .field("MPSS", &self.mpss().get())
            .field("FR", &self.fr().get())
            .field("CR", &self.cr().get())
            .field("CPS", &self.cps().get())
            // .field("PMA", &self.pma())
            .field("HPCP", &self.hpcp().get())
            .field("MPSP", &self.mpsp().get())
            .field("CPD", &self.cpd_ro().get())
            .field("ESP", &self.esp().get())
            .field("APSTE", &self.apste_ro().get())
            .field("ATAPI", &self.atapi_ro().get())
            .field("DLAE", &self.dlae_ro().get())
            // .field("ALPE", &self.alpe())
            // .field("ASP", &self.asp())
            // .field("ICC", &self.icc())
            .finish()
    }
}
