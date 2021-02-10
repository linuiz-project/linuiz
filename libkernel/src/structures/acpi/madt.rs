use x86_64::PhysAddr;

use crate::structures::acpi::{Checksum, SDTHeader};

#[repr(C)]
#[derive(Debug)]
pub struct MADT {
    header: SDTHeader,
    apic_addr: u32,
    flags: u32,
    irq_entry_head: InterruptDeviceHeader,
}

impl MADT {
    fn header_len(&self) -> usize {
        (self.header.len() as usize) - core::mem::size_of::<SDTHeader>() - 8
    }

    pub fn iter(&self) -> MADTIterator {
        let cur_entry_ptr = unsafe {
            core::mem::transmute::<&InterruptDeviceHeader, *const u8>(&self.irq_entry_head)
        };
        MADTIterator {
            cur_header_ptr: cur_entry_ptr,
            max_header_ptr: unsafe { cur_entry_ptr.offset(self.header_len() as isize) },
        }
    }
}

impl Checksum for MADT {
    fn bytes_len(&self) -> usize {
        self.header.len() as usize
    }
}

pub struct MADTIterator {
    cur_header_ptr: *const u8,
    max_header_ptr: *const u8,
}

impl Iterator for MADTIterator {
    type Item = InterruptDevice;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_header_ptr < self.max_header_ptr {
            unsafe {
                let header_ptr = self.cur_header_ptr as *const InterruptDeviceHeader;
                let header = &*header_ptr;
                self.cur_header_ptr = self.cur_header_ptr.offset(header.len as isize);

                match header.ty {
                    0x0 => Some(InterruptDevice::LocalAPIC(
                        *(header_ptr as *const LocalAPIC),
                    )),
                    0x1 => Some(InterruptDevice::IOAPIC(*(header_ptr as *const IOAPIC))),
                    0x2 => Some(InterruptDevice::IRQSrcOverride(
                        *(header_ptr as *const IRQSrcOverride),
                    )),
                    0x4 => Some(InterruptDevice::NonMaskableIRQ(
                        *(header_ptr as *const NonMaskableIRQ),
                    )),
                    0x5 => Some(InterruptDevice::LocalAPICAddrOverride(
                        *(header_ptr as *const LocalAPICAddrOverride),
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
pub enum InterruptDevice {
    LocalAPIC(LocalAPIC),
    IOAPIC(IOAPIC),
    IRQSrcOverride(IRQSrcOverride),
    NonMaskableIRQ(NonMaskableIRQ),
    LocalAPICAddrOverride(LocalAPICAddrOverride),
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
#[derive(Debug, Clone, Copy)]
pub struct IOAPIC {
    header: InterruptDeviceHeader,
    id: u8,
    reserved: [u8; 1],
    addr: u32,
    global_sys_interrupt_base: u32,
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
