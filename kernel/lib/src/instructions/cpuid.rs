#[derive(Debug)]
pub struct Registers {
    eax: u32,
    ebx: u32,
    ecx: u32,
    edx: u32,
}

impl Registers {
    pub const fn eax(&self) -> u32 {
        self.eax
    }

    pub const fn ebx(&self) -> u32 {
        self.ebx
    }

    pub const fn ecx(&self) -> u32 {
        self.ecx
    }

    pub const fn edx(&self) -> u32 {
        self.edx
    }
}

#[inline]
pub fn exec(leaf: u32, subleaf: u32) -> Option<Registers> {
    let (eax, ebx, ecx, edx);

    unsafe {
        core::arch::asm!(
            "push rbx",
            "cpuid",
            "xchg rbx, rsi",
            "pop rbx",
            inout("eax") leaf => eax,
            inout("ecx") subleaf => ecx,
            lateout("esi") ebx,
            lateout("edx") edx,
            options(nomem)
        )
    }

    if (eax | ebx | ecx | edx) > 0 {
        Some(Registers { eax, ebx, ecx, edx })
    } else {
        None
    }
}
