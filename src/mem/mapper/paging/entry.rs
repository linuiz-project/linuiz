use crate::mem::{
    Permissions,
    addr::phys::{FrameAddress, PhysicalAddress},
};
use bit_field::BitField;
use core::num::NonZero;

#[repr(transparent)]
#[derive(Default, Clone, PartialEq, Eq)]
pub struct Entry(usize);

impl Entry {
    #[cfg(target_arch = "x86_64")]
    const HUGE_BIT_INDEX: usize = 7;
    #[cfg(target_arch = "x86_64")]
    const NO_EXECUTE_BIT_INDEX: usize = 63;
    #[cfg(target_arch = "riscv64")]
    const READABLE_BIT_INDEX: usize = 1;
    #[cfg(target_arch = "riscv64")]
    const EXECUTABLE_BIT_INDEX: usize = 3;

    const PRESENT_BIT_INDEX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 0 }
            target_arch = "riscv64" => { 0 }
            _ => { unimplemented!() }
        }
    };

    const WRITABLE_BIT_INDEX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 1 }
            target_arch = "riscv64" => { 1 }
            _ => { unimplemented!() }
        }
    };

    const USER_BIT_INDEX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 2 }
            target_arch = "riscv64" => { 4 }
            _ => { unimplemented!() }
        }
    };

    const GLOBAL_BIT_INDEX: usize = {
        cfg_select! {
            target_arch = "x86_64" => { 8 }
            target_arch = "riscv64" => { 5 }
            _ => { unimplemented!() }
        }
    };

    const FRAME_BIT_MASK: NonZero<usize> = {
        cfg_select! {
            target_arch = "x86_64" => {
                NonZero::<usize>::new(0xF_FFFF_FFFF_F000).unwrap()
            }

            _ => { unimplemented!() }
        }
    };

    const FRAME_BIT_SHIFT: NonZero<u32> = {
        cfg_select! {
            target_arch = "x86_64" => {
                NonZero::<u32>::new(12).unwrap()
            }

            _ => { unimplemented!() }
        }
    };

    pub const fn empty() -> Self {
        Self(0)
    }

    pub unsafe fn clear(&mut self) {
        self.0 = 0;
    }

    /// Enables the memory region of this entry.
    pub fn is_enabled(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::PRESENT_BIT_INDEX)
            }

            target_arch = "riscv64" => {
                self.0.get_bit(Self::VALID_BIT_INDEX)
            }

            _ => { unimplemented!() }
        }
    }

    pub fn set_enabled(&mut self) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::PRESENT_BIT_INDEX, true);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::VALID_BIT_INDEX, true);
            }

            _ => { unimplemented!() }
        }
    }

    /// Disables the memory region of this entry.
    ///
    /// # Safety
    ///
    /// - Disabling a page table entry may cause a `#PF` if the memory is still
    ///   in use.
    pub unsafe fn set_disabled(&mut self) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::PRESENT_BIT_INDEX, false);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::VALID_BIT_INDEX, false);
            }

            _ => { unimplemented!() }
        }
    }

    /// Gets the address stored in this entry.
    pub fn get_address(&self) -> Option<PhysicalAddress> {
        if self.is_enabled() {
            let address = {
                cfg_select! {
                    target_arch = "x86_64" => {
                        self.0 & Self::FRAME_BIT_MASK.get()
                    }

                    _ => { unimplemented!() }
                }
            };

            // Safety: `address` is checked to only contain canonical physical bits.
            let address = unsafe { PhysicalAddress::new_unchecked(address) };

            Some(address)
        } else {
            None
        }
    }

    /// Sets the entry's frame index.
    ///
    /// # Safety
    ///
    /// - `frame` must be unused or otherwise expected to be pointed to by this
    ///   entry's address.
    pub unsafe fn set_address<F: FrameAddress>(&mut self, frame: F) {
        let address: usize = frame.into();
        debug_assert_eq!(address & !Self::FRAME_BIT_MASK.get(), 0);

        self.0 = (self.0 & !Self::FRAME_BIT_MASK.get()) | address;
    }

    pub fn is_global(&self) -> bool {
        cfg_select! {
            all(target_arch = "x86_64", test) => {
                self.0.get_bit(Self::GLOBAL_BIT_INDEX)
            }

            all(target_arch = "x86_64", not(test)) => {
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

            _ => { unimplemented!() }
        }
    }

    pub fn set_global(&mut self, global: bool) {
        cfg_select! {
            all(target_arch = "x86_64", test) => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
            }

            all(target_arch = "x86_64", not(test)) => {
                use crate::arch::x86_64::registers::control::cr4;

                if cr4::CR4::read().contains(cr4::Flags::PGE) {
                    self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
                } else {
                    // We don't really care if it's set if it isn't supported.
                    // Allowing this means it's much easier to manage the global
                    // bit across different platforms.
                }
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, global);
            }

            _ => { unimplemented!() }
        }
    }

    pub fn is_user(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => { self.0.get_bit(Self::USER_BIT_INDEX) }
            target_arch = "riscv64" => { self.0.get_bit(Self::USER_BIT_INDEX) }

            _ => { unimplemented!() }
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

            _ => { unimplemented!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn is_huge(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.get_bit(Self::HUGE_BIT_INDEX)
            }

            _ => { unimplemented!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn set_huge(&mut self, huge_page: bool) {
        cfg_select! {
            target_arch = "x86_64" => {
                self.0.set_bit(Self::HUGE_BIT_INDEX, huge_page);
            }

            _ => { unimplemented!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    /// Sets the `WRITE` bit, but not the `NO_EXECUTE` bit.
    ///
    /// # Remarks
    ///
    /// Setting the `WRITE` bit without the `NO_EXECUTE` bit is extremely
    /// unsafe. This should **only** be done for intermediate, non-leaf page
    /// table entries.
    pub fn set_write_execute(&mut self) {
        self.0.set_bit(Self::WRITABLE_BIT_INDEX, true);

        cfg_select! {
            test => {
                self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, false);
            }

            not(test) => {
                if crate::arch::x86_64::registers::model_specific::IA32_EFER::get_no_execute_enable() {
                    self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, false);
                }
            }
        }
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

                    // This should ONLY be used for intermediate entries.
                    (true, false) => Permissions::WriteExecute,
                }
            }

            _ => { unimplemented!() }
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

            _ => { unimplemented!() }
        }
    }

    /// `true` if the entry is an intermediate entry, or `false` if it's a leaf.
    pub fn is_intermediate(&self) -> bool {
        cfg_select! {
            target_arch = "x86_64" => { !self.is_huge() }

            _ => { unimplemented!() }
        }
    }
}

impl core::fmt::Debug for Entry {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let mut d = formatter.debug_struct("Entry");

        d.field("Enabled", &self.is_enabled());

        if let Some(address) = self.get_address() {
            d.field("Address", &address);
        }

        #[cfg(target_arch = "x86_64")]
        d.field("Huge", &self.is_huge());

        d.field("Global", &self.is_global())
            .field("Access", &self.get_permissions())
            .field("User", &self.is_user())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Entry;
    use crate::mem::Permissions;
    use libsys::address::{Address, Frame};

    #[test]
    pub fn default() {
        assert_eq!(Entry::default(), Entry(0));
    }

    #[test]
    pub fn enabled() {
        let mut entry = Entry::default();

        entry.set_enabled();
        assert_eq!(entry, Entry(1 << 0));

        // Safety: Entry not in use.
        unsafe {
            entry.set_disabled();
        }
        assert_eq!(entry, Entry(0));
    }

    #[test]
    pub fn frame() {
        const ADDRESS: usize = 0xFFF000;

        let mut entry = Entry::default();
        assert_eq!(entry, Entry(0));

        // Safety: Address is canonical.
        let frame = unsafe { Address::<Frame>::new_unchecked(ADDRESS) };
        // Safety: Entry not in use.
        unsafe {
            entry.set_address(frame);
        }
        assert_eq!(entry, Entry(ADDRESS));

        entry.set_enabled();
        assert_eq!(entry.get_address(), Some(frame));

        // Safety: Entry not in use.
        unsafe {
            entry.clear();
        }
        assert_eq!(entry, Entry(0));
    }

    #[test]
    fn global() {
        let mut entry = Entry::default();
        entry.set_global(true);

        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(1 << 8));
                assert_eq!(entry.is_global(), true);
            }
        }

        entry.set_global(false);
        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(0));
                assert_eq!(entry.is_global(), false);
            }
        }
    }

    #[test]
    fn user() {
        let mut entry = Entry::default();
        entry.set_user(true);

        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(1 << 2));
                assert_eq!(entry.is_user(), true);
            }
        }

        entry.set_user(false);
        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(0));
                assert_eq!(entry.is_user(), false);
            }
        }
    }

    #[test]
    fn huge() {
        let mut entry = Entry::default();
        entry.set_huge(true);

        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(1 << 7));
                assert_eq!(entry.is_huge(), true);
            }
        }

        entry.set_huge(false);
        cfg_select! {
            target_arch = "x86_64" => {
                assert_eq!(entry, Entry(0));
                assert_eq!(entry.is_huge(), false);
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn set_write_execute() {
        let mut entry = Entry::default();

        // Safety: Entry not in use.
        unsafe {
            entry.set_permissions(Permissions::ReadExecute);
        }
        assert_eq!(entry, Entry(0));

        entry.set_write_execute();
        assert_eq!(entry, Entry(1 << 1));
    }

    // TODO test `Entry::get/set _permissions`
}
