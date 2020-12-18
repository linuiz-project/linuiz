use crate::{
    boot::pic::{end_of_interrupt, InterruptOffset},
    write,
};
use lazy_static::lazy_static;
use x86_64::structures::idt::InterruptStackFrame;

pub(super) extern "x86-interrupt" fn timer_interrupt_handler(_: &mut InterruptStackFrame) {
    end_of_interrupt(InterruptOffset::Timer);
}

pub(super) extern "x86-interrupt" fn keyboard_interrupt_handler(_: &mut InterruptStackFrame) {
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use spin::Mutex;
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> = Mutex::new(
            Keyboard::new(layouts::Us104Key, ScancodeSet1, HandleControl::Ignore)
        );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port: Port<u8> = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => write!("{}", character),
                DecodedKey::RawKey(key) => write!("{:?}", key),
            }
        }
    }

    end_of_interrupt(InterruptOffset::Keyboard);
}
