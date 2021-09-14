use crate::{addr_ty::Virtual, Address};

macro_rules! basic_register {
    ($register_ident:ident, $register_asm:literal) => {
        pub struct $register_ident;


        impl $register_ident {
            #[inline(always)]
            pub unsafe fn write(value: Address<Virtual>) {
                asm!(concat!("mov ", $register_asm, ", {}"), in(reg) value.as_usize(), options(nomem, nostack));
            }

            #[inline(always)]
            pub unsafe fn add(count: u64) {
                asm!(concat!("add ", $register_asm, ", {}"), in(reg) count, options(nomem, nostack));
            }

            #[inline(always)]
            pub unsafe fn sub(count: u64) {
                asm!(concat!("sub ", $register_asm, ", {}"), in(reg) count, options(nomem, nostack));
            }

            #[inline(always)]
            pub fn read() -> Address<Virtual> {
                let addr: u64;
                unsafe {
                    asm!(concat!("mov {}, ", $register_asm), out(reg) addr, options(nomem, nostack));
                    Address::new_unsafe(addr as usize)
                }
            }
        }
    }
}

basic_register! {RSP, "rsp"}
basic_register! {RBP, "rbp"}

// pub struct RBP;

// pub struct RSP;

// impl RSP {
//     #[inline(always)]
//     pub unsafe fn write(value: Address<Virtual>) {
//         asm!("mov rsp, {}", in(reg) value.as_usize(), options(nomem, nostack));
//     }

//     #[inline(always)]
//     pub unsafe fn add(count: u64) {
//         asm!("add rsp, {}", in(reg) count, options(nomem, nostack));
//     }

//     #[inline(always)]
//     pub unsafe fn sub(count: u64) {
//         asm!("sub rsp, {}", in(reg) count, options(nomem, nostack));
//     }

//     #[inline(always)]
//     pub fn read() -> Address<Virtual> {
//         let addr: u64;
//         unsafe {
//             asm!("mov {}, rsp", out(reg) addr, options(nomem, nostack));
//             Address::new_unsafe(addr as usize)
//         }
//     }
// }
