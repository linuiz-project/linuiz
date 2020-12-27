use crate::io::port::{ReadWritePort, WriteOnlyPort};

const CMD_INIT: u8 = 0x11;
const CMD_END_OF_INTERRUPT: u8 = 0x20;
const MODE_8806: u8 = 0x01;

struct PIC {
    offset: u8,
    command: WriteOnlyPort<u8>,
    data: ReadWritePort<u8>,
}

impl PIC {
    fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        interrupt_id >= self.offset && interrupt_id < (self.offset + 8)
    }

    fn end_of_interrupt(&mut self) {
        self.command.write(CMD_END_OF_INTERRUPT);
    }
}

/// A pair of chained PIC controllers.
///
/// Remark: This is the standard setup on x86.
pub struct ChainedPICs {
    pics: [PIC; 2],
}

impl ChainedPICs {
    /// Create a new interface for the standard PIC1 and PIC2 controllers, specifying the desired interrupt offsets.
    pub const unsafe fn new(offset1: u8, offset2: u8) -> Self {
        Self {
            pics: [
                PIC {
                    offset: offset1,
                    command: WriteOnlyPort::new(0x20),
                    data: ReadWritePort::new(0x21),
                },
                PIC {
                    offset: offset2,
                    command: WriteOnlyPort::new(0xA0),
                    data: ReadWritePort::new(0xA1),
                },
            ],
        }
    }

    /// Initializes the chained PICs. They're initialized together (at the same time) because
    /// I/O operations might not be intantaneous on older processors.
    pub unsafe fn init(&mut self) {
        // We need to add a delay bettween writes to the PICs, especially on older motherboards.
        // This is because the PIC may not be fast enough to react to the previous command before
        // the next is sent.
        //
        // Additionally, at this point we don't necessarily have any kind of timer yet, because they
        // tend to required interrupts. This is usually worked around by writing garbage data to port 0x80,
        // which should take long enough to make everything work (* on most hardware).
        let mut io_wait_port = WriteOnlyPort::<u8>::new(0x80);
        let mut io_wait = || io_wait_port.write(0x0);

        // save the masks stored in data port
        let saved_mask0 = self.pics[0].data.read();
        let saved_mask1 = self.pics[1].data.read();

        // Tell each PIC that we're going to send it a 3-byte initialization sequence on its data port.
        self.pics[0].command.write(CMD_INIT);
        io_wait();
        self.pics[1].command.write(CMD_INIT);
        io_wait();

        // Assign the relevant offsets to each PIC in the chain.
        self.pics[0].data.write(self.pics[0].offset);
        io_wait();
        self.pics[1].data.write(self.pics[1].offset);
        io_wait();

        // Configure chaining between PICs 1 & 2.
        self.pics[0].data.write(4);
        io_wait();
        self.pics[1].data.write(2);
        io_wait();

        // Inform the PIC of what mode we'll be using them in.
        self.pics[0].data.write(MODE_8806);
        io_wait();
        self.pics[1].data.write(MODE_8806);
        io_wait();

        // Restore saved masks
        self.pics[0].data.write(saved_mask0);
        self.pics[1].data.write(saved_mask1);
    }

    // Indicates whether any of the chained PICs handle the given interrupt.
    pub fn handles_interrupt(&self, interrupt_id: u8) -> bool {
        self.pics
            .iter()
            .any(|pic| pic.handles_interrupt(interrupt_id))
    }

    pub unsafe fn end_of_interrupt(&mut self, interrupt_id: u8) {
        if self.handles_interrupt(interrupt_id) {
            // If the interrupt belongs to the slave PIC, we send the EOI command to it.
            if self.pics[1].handles_interrupt(interrupt_id) {
                self.pics[1].end_of_interrupt();
            }

            // No matter which PIC the interrupt belongs to, the EOI command must be sent
            // to the master PIC.
            // This is because the slave PIC is chained through the master PIC, so any interrupts
            // raise on the master as well.
            self.pics[0].end_of_interrupt();
        }
    }
}
