use crate::{
    acpi::{rdsp::xsdt, ACPITable, UnsizedACPITable},
    addr_ty::Physical,
    Address,
};

bitflags::bitflags! {
    pub struct Flags: u32 {
        const PCAT_COMPAT = 1 << 0;
    }
}

#[repr(C)]
pub struct Header {
    sdt_header: xsdt::SDTHeader,
    apic_addr: u32,
    flags: Flags,
}

pub struct MADT {}

impl xsdt::SubTable for MADT {
    const SIGNATURE: &'static str = &"APIC";
}

impl MADT {
    fn madt_header(&self) -> &Header {
        unsafe { (self as *const _ as *const Header).as_ref().unwrap() }
    }

    pub fn apic_addr(&self) -> Address<Physical> {
        Address::<Physical>::new(self.madt_header().apic_addr as usize)
    }

    pub fn flags(&self) -> &Flags {
        &self.madt_header().flags
    }

    pub fn iter(&self) -> MADTIterator {
        let first_entry_ptr = self.first_entry_ptr();
        MADTIterator {
            cur_header_ptr: first_entry_ptr,
            max_header_ptr: unsafe { first_entry_ptr.add(self.body_len()) },
            phantom: core::marker::PhantomData,
        }
    }
}

impl UnsizedACPITable<Header, u8> for MADT {}

pub struct MADTIterator<'a> {
    cur_header_ptr: *const u8,
    max_header_ptr: *const u8,
    phantom: core::marker::PhantomData<&'a u8>,
}

impl<'a> Iterator for MADTIterator<'a> {
    type Item = InterruptDevice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_header_ptr < self.max_header_ptr {
            unsafe {
                let header_ptr = self.cur_header_ptr as *const InterruptDeviceHeader;
                let header = header_ptr.as_ref().unwrap();
                self.cur_header_ptr = self.cur_header_ptr.add(header.len as usize);

                match header.ty {
                    0x0 => Some(InterruptDevice::LocalAPIC(
                        (header_ptr as *const LocalAPIC).as_ref().unwrap(),
                    )),
                    0x1 => Some(InterruptDevice::IOAPIC(
                        (header_ptr as *const IOAPIC).as_ref().unwrap(),
                    )),
                    0x2 => Some(InterruptDevice::IRQSrcOverride(
                        (header_ptr as *const IRQSrcOverride).as_ref().unwrap(),
                    )),
                    0x4 => Some(InterruptDevice::NonMaskableIRQ(
                        (header_ptr as *const NonMaskableIRQ).as_ref().unwrap(),
                    )),
                    0x5 => Some(InterruptDevice::LocalAPICAddrOverride(
                        (header_ptr as *const LocalAPICAddrOverride)
                            .as_ref()
                            .unwrap(),
                    )),
                    0xF..0x7F | 0x80..0xFF => Some(InterruptDevice::Reserved),
                    ty => panic!("invalid interrupt device type: 0x{:X}", ty),
                }
            }
        } else {
            None
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
struct InterruptDeviceHeader {
    ty: u8,
    len: u8,
}

#[derive(Clone, Copy)]
pub enum InterruptDevice<'a> {
    LocalAPIC(&'a LocalAPIC),
    IOAPIC(&'a IOAPIC),
    IRQSrcOverride(&'a IRQSrcOverride),
    NonMaskableIRQ(&'a NonMaskableIRQ),
    LocalAPICAddrOverride(&'a LocalAPICAddrOverride),
    Reserved,
}

impl core::fmt::Debug for InterruptDevice<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_tuple("InterruptDevice")
            .field(match self {
                InterruptDevice::LocalAPIC(_) => &"Local APIC",
                InterruptDevice::IOAPIC(_) => &"IO APIC",
                InterruptDevice::IRQSrcOverride(_) => &"IRQ Source Override",
                InterruptDevice::NonMaskableIRQ(_) => &"Non-Maskable IRQ",
                InterruptDevice::LocalAPICAddrOverride(_) => &"Local APIC Address Override",
                InterruptDevice::Reserved => &"Unhandled",
            })
            .finish()
    }
}

bitflags::bitflags! {
    pub struct LocalAPICFlags: u8 {
        const PROCESSOR_ENABLED = 1 << 0;
        const ONLINE_CAPABLE = 1 << 1;
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct LocalAPIC {
    header: InterruptDeviceHeader,
    processor_id: u8,
    id: u8,
    flags: LocalAPICFlags,
}

impl LocalAPIC {
    pub const fn processor_id(&self) -> u8 {
        self.processor_id
    }

    pub const fn id(&self) -> u8 {
        self.id
    }

    pub const fn flags(&self) -> LocalAPICFlags {
        self.flags
    }
}

impl core::fmt::Debug for LocalAPIC {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("Local APIC Descriptor")
            .field("Processor ID", &self.processor_id())
            .field("ID", &self.id())
            .field("Flags", &self.flags())
            .finish()
    }
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct IOAPIC {
    header: InterruptDeviceHeader,
    id: u8,
    reserved: [u8; 1],
    addr: u32,
    global_sys_interrupt_base: u32,
}

impl IOAPIC {
    pub fn id(&self) -> u8 {
        self.id
    }

    pub fn register_base(&self) -> Address<Physical> {
        Address::<Physical>::new(self.addr as usize)
    }

    pub fn read(&self, register: u8) -> u32 {
        unsafe {
            let ioapic_ptr = self.register_base().as_usize() as *mut u32;

            ioapic_ptr.write_volatile(register as u32);
            ioapic_ptr.offset(4).read_volatile()
        }
    }

    pub fn write(&self, register: u8, value: u32) {
        unsafe {
            let ioapic_ptr = self.register_base().as_usize() as *mut u32;

            ioapic_ptr.write_volatile(register as u32);
            ioapic_ptr.offset(4).write_volatile(value);
        }
    }
}

impl core::fmt::Debug for IOAPIC {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("IO APIC Device")
            .field("ID", &self.id())
            .field("Register Base", &self.register_base())
            .finish()
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct IRQSrcOverride {
    header: InterruptDeviceHeader,
    bus_src: u8,
    irq_src: u8,
    global_sys_interrupt: u32,
    flags: u16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct NonMaskableIRQ {
    header: InterruptDeviceHeader,
    processor_id: u8,
    flags: u16,
    lint_num: u8,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LocalAPICAddrOverride {
    header: InterruptDeviceHeader,
    reserved: [u8; 2],
    local_apic_addr: Address<Physical>,
}
