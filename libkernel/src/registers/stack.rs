use crate::{addr_ty::Virtual, Address};

pub struct RSP;

impl RSP {
    #[inline(always)]
    pub unsafe fn write(value: Address<Virtual>) {
        asm!("mov rsp, {}", in(reg) value.as_usize(), options(nomem, nostack));
    }

    #[inline(always)]
    pub unsafe fn add(count: u64) {
        asm!("add rsp, {}", in(reg) count, options(nomem, nostack));
    }

    #[inline(always)]
    pub unsafe fn sub(count: u64) {
        asm!("sub rsp, {}", in(reg) count, options(nomem, nostack));
    }

    #[inline(always)]
    pub fn read() -> Address<Virtual> {
        let addr: u64;
        unsafe {
            asm!("mov {}, rsp", out(reg) addr, options(nomem, nostack));
            Address::new_unsafe(addr as usize)
        }
    }
}
