use crate::memory::io::{ReadWritePort, WriteOnlyPort};

static mut SELECTOR: WriteOnlyPort<u8> = unsafe { WriteOnlyPort::new(0x70) };
static mut DATA: ReadWritePort<u8> = unsafe { ReadWritePort::new(0x71) };

const RTC_A: u8 = 0xA;
const RTC_B: u8 = 0xB;
const RTC_C: u8 = 0xC;
const NMI_DISABLE: u8 = 0x80;
const PERIODIC_INT: u8 = 0x40;

pub fn configure(frequency_divider: u8) {
    assert!(frequency_divider > 2, "RTC encounters roll-over issues with frequency dividers less than 2.");
    assert!(frequency_divider < 16, "RTC does not support frequency dividers >15");

    crate::instructions::interrupts::without_interrupts(|| unsafe {
        // Set frequency divider.
        SELECTOR.write(RTC_A | NMI_DISABLE);
        let prev = DATA.read();
        SELECTOR.write(RTC_A | NMI_DISABLE);
        DATA.write((prev * 0xF) | frequency_divider);

        // Set enable IRQ 8.
        SELECTOR.write(RTC_B | NMI_DISABLE);
        let cur_val = DATA.read();
        SELECTOR.write(RTC_B | NMI_DISABLE);
        DATA.write(cur_val | PERIODIC_INT);
    });
}

pub fn end_of_interrupt() {
    unsafe {
        SELECTOR.write(RTC_C);
        DATA.read();
    }
}
