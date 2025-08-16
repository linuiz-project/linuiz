use crate::mem::Permissions;
use bit_field::BitField;
use libsys::address::{Address, Frame};

#[repr(transparent)]
#[derive(Clone)]
pub struct Entry(usize);

impl Entry {
    #[cfg(target_arch = "x86_64")]
    const PRESENT_BIT_INDEX: usize = 0;
    #[cfg(target_arch = "x86_64")]
    const WRITABLE_BIT_INDEX: usize = 1;
    #[cfg(target_arch = "x86_64")]
    const USER_BIT_INDEX: usize = 2;
    #[cfg(target_arch = "x86_64")]
    const HUGE_BIT_INDEX: usize = 7;
    #[cfg(target_arch = "x86_64")]
    const GLOBAL_BIT_INDEX: usize = 8;
    #[cfg(target_arch = "x86_64")]
    const NO_EXECUTE_BIT_INDEX: usize = 63;

    #[cfg(target_arch = "riscv64")]
    const VALID_BIT_INDEX: usize = 0;
    #[cfg(target_arch = "riscv64")]
    const READABLE_BIT_INDEX: usize = 1;
    #[cfg(target_arch = "riscv64")]
    const WRITABLE_BIT_INDEX: usize = 2;
    #[cfg(target_arch = "riscv64")]
    const EXECUTABLE_BIT_INDEX: usize = 3;
    #[cfg(target_arch = "riscv64")]
    const USER_BIT_INDEX: usize = 4;
    #[cfg(target_arch = "riscv64")]
    const GLOBAL_BIT_INDEX: usize = 5;

    fn get_frame_address_range() -> core::ops::Range<usize> {
        static FRAME_ADDRESS_RANGE: spin::Lazy<core::ops::Range<usize>> = spin::Lazy::new(|| {
            cfg_select! {
                any(target_arch = "x86", target_arch = "x86_64") => {
                    use crate::arch::x86_64::{cpuid::feature_info, registers::control::cr4};
                    use raw_cpuid::FeatureInfo;

                    if feature_info().is_some_and(FeatureInfo::has_pae)
                        && cr4::CR4::read().contains(cr4::Flags::PAE)
                    {

                        12..51
                    } else {
                        12..32
                    }
                }

                _ => { todo!() }
            }
        });

        FRAME_ADDRESS_RANGE.clone()
    }

    pub const fn empty() -> Self {
        Self(0)
    }

    pub unsafe fn clear(&mut self) {
        self.0 = 0;
    }

    /// Gets the frame index of the page table entry.
    pub fn get_frame(&self) -> Option<Address<Frame>> {
        if !self.is_enabled() {
            return None;
        }

        let frame_index = self.0.get_bits(Self::get_frame_address_range());
        let frame_address =
            Address::<Frame>::from_index(frame_index).expect("entry's frame address is invalid");

        Some(frame_address)
    }

    /// Sets the entry's frame index.
    ///
    /// # Safety
    ///
    /// - `frame` must be unused or otherwise expected to be pointed to by this
    ///   entry's address.
    pub unsafe fn set_frame(&mut self, frame: Address<Frame>) {
        self.0
            .set_bits(Self::get_frame_address_range(), frame.index());
    }

    pub fn is_enabled(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::PRESENT_BIT_INDEX)
            }

            target_arch = "riscv64" => {
                self.0.get_bit(Self::VALID_BIT_INDEX)
            }

            _ => { todo!() }
        }
    }

    /// Enables or disables the memory region this entry represents.
    ///
    /// # Safety
    ///
    /// - Disabling a page table entry may cause a `#PF` if the memory is still
    ///   in use.
    pub unsafe fn set_enabled(&mut self, enabled: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::PRESENT_BIT_INDEX, enabled);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::VALID_BIT_INDEX, enabled);
            }

            _ => { todo!() }
        }
    }

    pub fn is_global(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::control::cr4;

                if cr4::CR4::read().contains(cr4::Flags::PGE) {
                    self.0.get_bit(Self::GLOBAL_BIT_INDEX)
                } else {
                    false
                }
            }

            target_arch = "riscv64" => {
                self.0.get_bit(Self::GLOBAL_BIT_INDEX)
            }

            _ => { todo!() }
        }
    }

    pub fn set_global(&mut self, global: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::control::cr4;

                if cr4::CR4::read().contains(cr4::Flags::PGE) {
                    self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
                }
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
            }

            _ => { todo!() }
        }
    }

    pub fn is_user(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => { self.0.get_bit(Self::USER_BIT_INDEX) }
            target_arch = "riscv64" => { self.0.get_bit(Self::USER_BIT_INDEX) }

            _ => { todo!() }
        }
    }

    pub fn set_user(&mut self, user_accessible: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, user_accessible);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, user_accessible);
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn is_huge(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::HUGE_BIT_INDEX)
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn set_huge(&mut self, huge_page: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::HUGE_BIT_INDEX, huge_page);
            }

            _ => { todo!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    /// Sets the `WRITE` bit, but no the `NO_EXECUTE` bit.
    ///
    /// # Remarks
    ///
    /// Setting the `WRITE` bit without the `NO_EXECUTE` bit is extremely
    /// unsafe.T his should **only** be done for intermediate, non-leaf page
    /// table entries.
    pub fn set_write_execute(&mut self) {
        self.0.set_bit(Self::WRITABLE_BIT_INDEX, true);
        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, false);
    }

    pub fn get_permissions(&self) -> Permissions {
        cfg_select! {
            target_arch = "x86_64" => {
                match (
                    self.0.get_bit(Self::WRITABLE_BIT_INDEX),
                    self.0.get_bit(Self::NO_EXECUTE_BIT_INDEX),
                ) {
                    (false, true) => Permissions::ReadOnly,
                    (true, true) => Permissions::ReadWrite,
                    (false, false) => Permissions::ReadExecute,

                    // This should ONLY be for intermediate entries.
                    (true, false) => Permissions::WriteExecute,
                }
            }

            _ => { todo!() }
        }
    }

    pub unsafe fn set_permissions(&mut self, permissions: Permissions) {
        cfg_select! {
            target_arch = "x86_64" => {
                match permissions {
                    // All pages in x86 are at least read only, so
                    // `Permissions::None` is effectively analogous to that.
                    Permissions::None | Permissions::ReadOnly => {
                        self.0.set_bit(Self::WRITABLE_BIT_INDEX, false);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, true);
                    }

                    Permissions::ReadWrite => {
                        self.0.set_bit(Self::WRITABLE_BIT_INDEX, true);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, true);
                    }

                    Permissions::ReadExecute => {
                        self.0.set_bit(Self::WRITABLE_BIT_INDEX, false);
                        self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, false);
                    }

                    Permissions::WriteExecute => {
                        unimplemented!("use `::set_write_execute` to make an entry W/X");
                    }
                }
            }

            _ => { todo!() }
        }
    }
}

impl Default for Entry {
    fn default() -> Self {
        cfg_select! {
            target_arch = "x86_64" => {
                Self(1 << Self::WRITABLE_BIT_INDEX)
            }
        }
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut debug_struct = formatter.debug_struct("Entry");

        debug_struct.field("Enabled", &self.is_enabled()).field(
            "Physical Address",
            &self.get_frame().map(|frame| frame.get().get()),
        );

        #[cfg(target_arch = "x86_64")]
        debug_struct.field("Huge", &self.is_huge());

        debug_struct
            .field("Global", &self.is_global())
            .field("Permissions", &self.get_permissions())
            .field("User Accessible", &self.is_user())
            .finish()
    }
}
