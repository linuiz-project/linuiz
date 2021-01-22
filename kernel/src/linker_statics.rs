use core::ffi::c_void;

use x86_64::VirtAddr;

use crate::memory::{paging::VirtualAddressorCell, Page};

extern "C" {
    static _text_start: c_void;
    static _text_end: c_void;

    static _rodata_start: c_void;
    static _rodata_end: c_void;

    static _data_start: c_void;
    static _data_end: c_void;

    static _bss_start: c_void;
    static _bss_end: c_void;
}

pub fn validate_section_mappings(virtual_addressor: &VirtualAddressorCell) {
    fn validate_section(virtual_addressor: &VirtualAddressorCell, section: core::ops::Range<u64>) {
        for addr in section.step_by(0x1000) {
            let page = Page::from_addr(VirtAddr::new(addr));
            if !virtual_addressor.is_mapped(&page) {
                panic!("failed to validate section: page {:?} not mapped.", page);
            }
        }
    }

    debug!("Validating all kernel sections are mapped for addressor.");

    let text_section = _text();
    debug!("Validating .text section ({:?})...", text_section);
    validate_section(virtual_addressor, text_section);

    let rodata_section = _rodata();
    debug!("Validating .rodata section ({:?})...", rodata_section);
    validate_section(virtual_addressor, rodata_section);

    let data_section = _data();
    debug!("Validating .data section ({:?})...", data_section);
    validate_section(virtual_addressor, data_section);

    let bss_section = _bss();
    debug!("Validating .bss section ({:?})...", bss_section);
    validate_section(virtual_addressor, bss_section);

    debug!("Validated all sections.");
}

pub fn _text() -> core::ops::Range<u64> {
    unsafe { (&_text_start as *const c_void as u64)..(&_text_end as *const c_void as u64) }
}

pub fn _rodata() -> core::ops::Range<u64> {
    unsafe { (&_rodata_start as *const c_void as u64)..(&_rodata_end as *const c_void as u64) }
}

pub fn _data() -> core::ops::Range<u64> {
    unsafe { (&_data_start as *const c_void as u64)..(&_data_end as *const c_void as u64) }
}

pub fn _bss() -> core::ops::Range<u64> {
    unsafe { (&_bss_start as *const c_void as u64)..(&_bss_end as *const c_void as u64) }
}
