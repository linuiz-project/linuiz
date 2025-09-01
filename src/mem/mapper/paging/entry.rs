use crate::{mem::Permissions, util::sync::Lazy};
use bit_field::BitField;
use core::ops::Range;
use libsys::address::{Address, Frame};

#[repr(transparent)]
#[derive(Default, Clone, PartialEq, Eq)]
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

    fn get_frame_address_range() -> Range<usize> {
        static FRAME_ADDRESS_RANGE: Lazy<Range<usize>> = Lazy::new(|| {
            cfg_select! {
                all(any(target_arch = "x86", target_arch = "x86_64"), test) => {
                    12..51
                }

                all(any(target_arch = "x86", target_arch = "x86_64"), not(test)) => {
                    use crate::arch::x86_64::{cpuid::feature_info, registers::control::cr4};

                    if feature_info().is_some_and(|cpuid| cpuid.has_pae())
                        && cr4::CR4::read().contains(cr4::Flags::PAE)
                    {
                        12..51
                    } else {
                        12..32
                    }
                }

                _ => { unimplemented!() }
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
        self.is_enabled().then(|| {
            let frame_index = self.0.get_bits(Self::get_frame_address_range());
            Address::<Frame>::from_index(frame_index).expect("entry's frame address is invalid")
        })
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

            _ => { unimplemented!() }
        }
    }

    /// Enables the memory region of this entry.
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

            _ => { unimplemented!() }
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

        if let Some(frame_address) = self.get_frame() {
            d.field("Address", &frame_address.get().get());
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
            entry.set_frame(frame);
        }
        assert_eq!(entry, Entry(ADDRESS));

        entry.set_enabled();
        assert_eq!(entry.get_frame(), Some(frame));

        // Safety: Entry not in use.
        unsafe {
            entry.clear();
        }
        assert_eq!(entry, Entry(0));
    }
}
