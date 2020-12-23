use crate::Address;

#[derive(Debug)]
pub struct CR2;

impl CR2 {
    pub fn read() -> Address {
        let value: usize;

        unsafe {
            asm!("mov {}, cr2", out(reg) value, options(nomem));
        }

        Address::Physical(value)
    }
}
