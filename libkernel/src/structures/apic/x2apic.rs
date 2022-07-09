use crate::registers::msr::{rdmsr, wrmsr};

#[repr(u32)]
pub enum APIC_MSR {
    ID = 0x802,
    VERSION = 0x803,
    TPR = 0x808,
    PPR = 0x80A,
    EOI = 0x80B,
    LDR = 0x80C,
    SPURIOUS = 0x80F,
    ISR0 = 0x810,
    ISR32 = 0x811,
    ISR64 = 0x812,
    ISR96 = 0x813,
    ISR128 = 0x814,
    ISR160 = 0x815,
    ISR192 = 0x816,
    ISR224 = 0x817,
    TMR0 = 0x818,
    TMR32 = 0x819,
    TMR64 = 0x81A,
    TMR96 = 0x81B,
    TMR128 = 0x81C,
    TMR160 = 0x81D,
    TMR192 = 0x81E,
    TMR224 = 0x81F,
    IRR0 = 0x820,
    IRR32 = 0x821,
    IRR64 = 0x822,
    IRR96 = 0x823,
    IRR128 = 0x824,
    IRR160 = 0x825,
    IRR192 = 0x826,
    IRR224 = 0x827,
    ERR = 0x828,
    ICR = 0x830,
    LVT_TIMER = 0x832,
    LVT_THERMAL = 0x833,
    LVT_PERF = 0x834,
    LVT_LINT0 = 0x835,
    LVT_LINT1 = 0x836,
    LVT_ERR = 0x837,
    TIMER_INT_CNT = 0x838,
    TIMER_CUR_CNT = 0x839,
    DIVIDE_CONF = 0x83E,
    SELF_IPI = 0x83F,
}

const MSR_BASE: u32 = 0x800;
pub struct x2APIC;

impl super::APIC for x2APIC {
    fn read_offset(offset: super::Offset) -> u64 {
        unsafe { rdmsr(MSR_BASE + (offset as u32)) }
    }

    unsafe fn write_offset(offset: super::Offset, value: u64) {
        wrmsr(MSR_BASE + (offset as u32), value);
    }

    unsafe fn send_int_cmd(int_cmd: super::InterruptCommand) {
        wrmsr(APIC_MSR::ICR as u32, int_cmd.get_raw());
    }
}
