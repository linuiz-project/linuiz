use x86_64::VirtAddr;

pub struct RSP;

impl RSP {
    #[inline(always)]
    pub unsafe fn write(value: VirtAddr) {
        asm!("mov rsp, {}", in(reg) value.as_u64(), options(nomem, nostack));
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
    pub fn read() -> VirtAddr {
        let addr: u64;
        unsafe {
            asm!("mov {}, rsp", out(reg) addr, options(nomem, nostack));
            VirtAddr::new_unsafe(addr)
        }
    }
}
