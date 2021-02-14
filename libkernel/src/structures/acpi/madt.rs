use crate::structures::{
    acpi::{Checksum, SDTHeader},
    apic::APIC,
};
use core::marker::PhantomData;
use x86_64::PhysAddr;

#[repr(C)]
#[derive(Debug)]
pub struct MADT {
    header: SDTHeader,
    apic_addr: u32,
    flags: u32,
    irq_entry_head: InterruptDeviceHeader,
}

impl MADT {
    fn body_len(&self) -> usize {
        (self.header.len() as usize) - core::mem::size_of::<SDTHeader>() - 8
    }

    pub fn apic(&self) -> APIC {
        for interrupt_device in self.iter() {
            if let InterruptDevice::LocalAPICAddrOverride(lapicaddr) = interrupt_device {
                return lapicaddr.apic();
            }
        }

        unsafe { APIC::from_addr(PhysAddr::new(self.apic_addr as u64)) }
    }

    pub fn iter(&self) -> MADTIterator {
        let cur_entry_ptr = (&self.irq_entry_head) as *const _ as *const u8;

        MADTIterator {
            cur_header_ptr: cur_entry_ptr,
            max_header_ptr: unsafe { cur_entry_ptr.offset(self.body_len() as isize) },
            phantom: PhantomData,
        }
    }
}

impl Checksum for MADT {
    fn bytes_len(&self) -> usize {
        self.header.len() as usize
    }
}

pub struct MADTIterator<'a> {
    cur_header_ptr: *const u8,
    max_header_ptr: *const u8,
    phantom: PhantomData<&'a u8>,
}

impl<'a> Iterator for MADTIterator<'a> {
    type Item = InterruptDevice<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_header_ptr < self.max_header_ptr {
            unsafe {
                let header_ptr = self.cur_header_ptr as *const InterruptDeviceHeader;
                let header = &*header_ptr;
                self.cur_header_ptr = self.cur_header_ptr.offset(header.len as isize);

                match header.ty {
                    0x0 => Some(InterruptDevice::LocalAPIC(
                        &*(header_ptr as *const LocalAPIC),
                    )),
                    0x1 => Some(InterruptDevice::IOAPIC(&*(header_ptr as *const IOAPIC))),
                    0x2 => Some(InterruptDevice::IRQSrcOverride(
                        &*(header_ptr as *const IRQSrcOverride),
                    )),
                    0x4 => Some(InterruptDevice::NonMaskableIRQ(
                        &*(header_ptr as *const NonMaskableIRQ),
                    )),
                    0x5 => Some(InterruptDevice::LocalAPICAddrOverride(
                        &*(header_ptr as *const LocalAPICAddrOverride),
                    )),
                    ty => panic!("invalid interrupt device type: {}", ty),
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

#[derive(Debug, Clone, Copy)]
pub enum InterruptDevice<'a> {
    LocalAPIC(&'a LocalAPIC),
    IOAPIC(&'a IOAPIC),
    IRQSrcOverride(&'a IRQSrcOverride),
    NonMaskableIRQ(&'a NonMaskableIRQ),
    LocalAPICAddrOverride(&'a LocalAPICAddrOverride),
}

bitflags::bitflags! {
    pub struct LocalAPICFlags: u8 {
        const PROCESSOR_ENABLED = 1 << 0;
        const ONLINE_CAPABLE = 1 << 1;
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct LocalAPIC {
    header: InterruptDeviceHeader,
    processor_id: u8,
    id: u8,
    flags: LocalAPICFlags,
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

    pub fn register_base(&self) -> PhysAddr {
        PhysAddr::new(self.addr as u64)
    }

    pub fn read(&self, register: u8) -> u32 {
        unsafe {
            let ioapic_ptr = self.register_base().as_u64() as *mut u32;

            ioapic_ptr.write_volatile(register as u32);
            ioapic_ptr.offset(4).read_volatile()
        }
    }

    pub fn write(&self, register: u8, value: u32) {
        unsafe {
            let ioapic_ptr = self.register_base().as_u64() as *mut u32;

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
    local_apic_addr: PhysAddr,
}

impl LocalAPICAddrOverride {
    pub fn apic(&self) -> APIC {
        unsafe { APIC::from_addr(self.local_apic_addr) }
    }
}
