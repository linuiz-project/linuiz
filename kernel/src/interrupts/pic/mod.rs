#![allow(dead_code)]

/*
Represents a wrapper around the hardware Programmable Interrupt Controller. This is an implementation
based around the Intel 8259 PIC, which is still supported in favor of backwards compatibility.

Information about the PIC can be found here: https://en.wikipedia.org/wiki/Intel_8259
*/

pub mod pit;

use crate::memory::io::{ReadWritePort, WriteOnlyPort};
use num_enum::TryFromPrimitive;

const CMD_INIT: u8 = 0x11;
const CMD_END_OF_INTERRUPT: u8 = 0x20;
const MODE_8086: u8 = 0x01;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive)]
pub enum InterruptOffset {
    Timer = 0,
    Keyboard = 1,
    COM2 = 2,
    COM1 = 3,
    LPT2 = 4,
    FloppyDisk = 5,
    SpuriousMaster = 6,
    RTC = 7,
    Peripheral0 = 8,
    Peripheral1 = 9,
    Peripheral2 = 10,
    PS2Mouse = 11,
    FPU = 12,
    PrimaryATA = 13,
    SpuriousSlave = 14,
}

bitflags::bitflags! {
    #[repr(transparent)]
    pub struct InterruptLines : u16 {
        const TIMER =           1 << 0;
        const KEYBOARD =        1 << 1;
        const COM2 =            1 << 3;
        const COM1 =            1 << 4;
        const LPT2 =            1 << 5;
        const FLOPPY_DISK =     1 << 6;
        const SPURIOUS_MASTER = 1 << 7;
        const RTC =             1 << 8;
        const PERIPHERAL0 =     1 << 9;
        const PERIPHERAL1 =     1 << 10;
        const PERIPHERAL2 =     1 << 11;
        const PS2_MOUSE =       1 << 12;
        const FPU =             1 << 13;
        const PRIMARY_ATA =     1 << 14;
        const SPURIOUS_SLAVE =  1 << 15;
    }
}

impl InterruptLines {
    /// Low bits of the interrupt lines.
    #[inline(always)]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn low(self) -> u8 {
        self.bits() as u8
    }

    /// High bits of the interrupt lines.
    #[inline(always)]
    #[allow(clippy::cast_possible_truncation)]
    pub const fn high(self) -> u8 {
        (self.bits() >> 8) as u8
    }

    /// All interrupt lines disabled.
    #[inline(always)]
    pub const fn disabled() -> Self {
        Self::empty()
    }
}

/// Simple PIC type for manipulating the master or slave PIC of the 8259 chained PICs.
struct PIC {
    offset: u8,
    command: WriteOnlyPort<u8>,
    data: ReadWritePort<u8>,
}

impl PIC {
    /// Returns whether or not the PIC handles the given interrupt.
    #[inline(always)]
    fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        interrupt_id >= self.offset && interrupt_id < (self.offset + 8)
    }

    /// Triggers an end of interrupt for the PIC.
    #[inline(always)]
    fn end_of_interrupt(&mut self) {
        self.command.write(CMD_END_OF_INTERRUPT);
    }
}

/// A pair of chained PIC controllers.
///
/// REMARK: This is the standard setup on x86.
pub struct PICS([PIC; 2]);

impl PICS {
    /// Create a new interface for the standard PIC1 and PIC2 controllers, specifying the desired interrupt offsets.
    const unsafe fn new(base_irq: u8) -> Self {
        Self([
            PIC { offset: base_irq, command: WriteOnlyPort::new(0x20), data: ReadWritePort::new(0x21) },
            PIC { offset: base_irq + 8, command: WriteOnlyPort::new(0xA0), data: ReadWritePort::new(0xA1) },
        ])
    }

    /// Initializes the chained PICs. They're initialized together (at the same time) because
    /// I/O operations might not be intantaneous on older processors.
    unsafe fn init(&mut self, enabled: InterruptLines) {
        // We need to add a delay bettween writes to the PICs, especially on older motherboards.
        // This is because the PIC may not be fast enough to react to the previous command before
        // the next is sent.
        //
        // Additionally, at this point we don't necessarily have any kind of timer yet, because they
        // tend to require interrupts. This is usually worked around by writing garbage data to port 0x80,
        // which should take long enough to make everything work (* on most hardware ?).
        let mut io_wait_port = WriteOnlyPort::<u8>::new(0x80);
        let mut io_wait = || io_wait_port.write(0x0);

        // Tell each PIC that we're going to send it a 3-byte initialization sequence on its data port.
        self.0[0].command.write(CMD_INIT);
        io_wait();
        self.0[1].command.write(CMD_INIT);
        io_wait();

        // Assign the relevant offsets to each PIC in the chain.
        self.0[0].data.write(self.0[0].offset);
        io_wait();
        self.0[1].data.write(self.0[1].offset);
        io_wait();

        // Configure chaining between PICs 1 & 2.
        self.0[0].data.write(1 << 2);
        io_wait();
        self.0[1].data.write(1 << 1);
        io_wait();

        // Inform the PIC of what mode we'll be using them in.
        self.0[0].data.write(MODE_8086);
        io_wait();
        self.0[1].data.write(MODE_8086);
        io_wait();

        // Write masks to data port, specifying which interrupts are ignored.
        self.0[0].data.write(!enabled.low() & !(1 << 2) /* never mask cascade */);
        io_wait();
        self.0[1].data.write(!enabled.high());
    }

    /// Indicates whether any of the chained PICs handle the given interrupt.
    fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.0[0].handles_interrupt(interrupt_id) || self.0[1].handles_interrupt(interrupt_id)
    }

    /// Signals to the chained PICs to send the EOI command.
    /// SAFETY: This function is unsafe because an invalid interrupt ID can be specified.
    unsafe fn end_of_interrupt(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            // If the interrupt belongs to the slave PIC, we send the EOI command to it.
            if self.0[1].handles_interrupt(interrupt_id) {
                self.0[1].end_of_interrupt();
            }

            // No matter which PIC the interrupt belongs to, the EOI command must be sent
            // to the master PIC.
            // This is because the slave PIC is chained through the master PIC, so any interrupts
            // raise on the master as well.
            self.0[0].end_of_interrupt();
        } else {
            trace!("Invalid EOI request: {}", interrupt_id);
        }
    }
}

// This is a lazy static to allow *not* initializing it on systems that don't
// support it (when software properly checks for support).
lazy_static::lazy_static! {
    static ref PIC8259: spin::Mutex<PICS> = spin::Mutex::new(unsafe { PICS::new(32) });
}

/// Sets the enabled interrupt lines for the 8259 PIC.
///
/// SAFETY: Setting new enabled interrupt lines has the possibility of adversely affecting control flow
///         unrelated to this function, or even this core's context. It is thus the responsibility of the
///         caller to ensure modifying the enabled lines will not result in unwanted behaviour.
pub unsafe fn set_enabled_lines(enabled_lines: InterruptLines) {
    PIC8259.lock().init(enabled_lines);
}

/// Safely sends an `end of interrupt` to the 8259 PIC.
pub fn end_of_interrupt(offset: InterruptOffset) {
    unsafe {
        PIC8259.lock().end_of_interrupt(offset as u8);
    }
}
