use crate::mem::{
    Permissions,
    addr::phys::{FrameAddress, PhysicalAddress, StandardFrame},
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

    fn non_address_low_bits() -> NonZero<u32> {
        cfg_select! {
            target_arch = "x86_64" => {
                // Safety: Value is non-zero.
                unsafe { NonZero::<u32>::new_unchecked(12) }
            }

            _ => { unimplemented!() }
        }
    }

    fn address_mask() -> NonZero<usize> {
        cfg_select! {
            target_arch = "x86_64" => {
                StandardFrame::canonical_mask()
            }

            _ => { unimplemented!() }
        }
    }

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
                        self.0 & Self::address_mask().get()
                    }

                    _ => { unimplemented!() }
                }
            };

            let address = address
                .unbounded_shr(Self::non_address_low_bits().get())
                .unbounded_shl(StandardFrame::INDEX_BIT_SHIFT.get());

            debug_assert!(PhysicalAddress::check_canonical(address));

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
        let address = {
            cfg_select! {
                target_arch = "x86_64" => { Into::<usize>::into(frame) }
                _ => { unimplemented!() }
            }
        };

        debug_assert_eq!(address & !Self::address_mask().get(), 0);

        let address = address
            .unbounded_shr(StandardFrame::INDEX_BIT_SHIFT.get())
            .unbounded_shl(Self::non_address_low_bits().get());

        self.0 = (self.0 & !Self::address_mask().get()) | address;
    }

    pub fn is_global(&self) -> bool {
        cfg_select! {
            test => {
                self.0.get_bit(Self::GLOBAL_BIT_INDEX)
            }

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

            _ => { unimplemented!() }
        }
    }

    pub fn set_global(&mut self, set: bool) {
        cfg_select! {
            test => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, set);
            }

            target_arch = "x86_64" => {
                use crate::arch::x86_64::registers::control::cr4;

                if cr4::CR4::read().contains(cr4::Flags::PGE) {
                    self.0.set_bit(Self::GLOBAL_BIT_INDEX, set);
                }
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::GLOBAL_BIT_INDEX, set);
            }

            _ => { unimplemented!() }
        }
    }

    pub fn is_user(&self) -> bool {
        cfg_select! {
            test => { self.0.get_bit(Self::USER_BIT_INDEX) }
            target_arch = "x86_64" => { self.0.get_bit(Self::USER_BIT_INDEX) }
            target_arch = "riscv64" => { self.0.get_bit(Self::USER_BIT_INDEX) }

            _ => { unimplemented!() }
        }
    }

    pub fn set_user(&mut self, set: bool) {
        cfg_select! {
            test => {
                self.0.set_bit(Self::USER_BIT_INDEX, set);
            }

            target_arch = "x86_64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, set);
            }

            target_arch = "riscv64" => {
                self.0.set_bit(Self::USER_BIT_INDEX, set);
            }

            _ => { unimplemented!() }
        }
    }

    #[cfg(target_arch = "x86_64")]
    pub fn is_huge(&self) -> bool {
        self.0.get_bit(Self::HUGE_BIT_INDEX)
    }

    #[cfg(target_arch = "x86_64")]
    pub fn set_huge(&mut self, set: bool) {
        self.0.set_bit(Self::HUGE_BIT_INDEX, set);
    }

    #[cfg(target_arch = "x86_64")]
    fn get_no_execute_bit(&self) -> bool {
        cfg_select! {
            test => {
                self.0.get_bit(Self::NO_EXECUTE_BIT_INDEX)
            }

            not(test) => {
                if crate::arch::x86_64::registers::model_specific::IA32_EFER::get_no_execute_enable() {
                    self.0.get_bit(Self::NO_EXECUTE_BIT_INDEX)
                } else {
                    false
                }
            }
        }
    }

    #[cfg(target_arch = "x86_64")]
    fn set_no_execute_bit(&mut self, set: bool) {
        cfg_select! {
            test => {
                self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, set);
            }

            not(test) => {
                if crate::arch::x86_64::registers::model_specific::IA32_EFER::get_no_execute_enable() {
                    self.0.set_bit(Self::NO_EXECUTE_BIT_INDEX, set);
                }
            }
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
        self.set_no_execute_bit(false);
    }

    pub fn get_permissions(&self) -> Permissions {
        cfg_select! {
            target_arch = "x86_64" => {
                match (
                    self.0.get_bit(Self::WRITABLE_BIT_INDEX),
                    self.get_no_execute_bit(),
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
                        self.set_no_execute_bit(true);
                    }

                    Permissions::ReadWrite => {
                        self.0.set_bit(Self::WRITABLE_BIT_INDEX, true);
                        self.set_no_execute_bit(true);
                    }

                    Permissions::ReadExecute => {
                        self.0.set_bit(Self::WRITABLE_BIT_INDEX, false);
                        self.set_no_execute_bit(false);
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

        if let Some(address) = self.get_address() {
            d.field("Address", &address);
        }

        #[cfg(target_arch = "x86_64")]
        d.field("Huge", &self.is_huge());

        d.field("Global", &self.is_global())
            .field("Access", &self.get_permissions())
            .field("User", &self.is_user())
            .field("Raw", &format_args!("{:#X}", self.0))
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::Entry;
    use crate::mem::{
        Permissions,
        addr::phys::{FrameAddress, PhysicalAddress, StandardFrame},
    };

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
        let frame = unsafe { StandardFrame::new_unchecked(ADDRESS) };
        // Safety: Entry not in use.
        unsafe {
            entry.set_address(frame);
        }
        assert_eq!(entry, Entry(ADDRESS));

        entry.set_enabled();
        assert_eq!(entry.get_address(), Some(PhysicalAddress::from(frame)));

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
