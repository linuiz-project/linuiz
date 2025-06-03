use bit_field::BitField;

const TICK_RATE: u32 = 1193182;

#[repr(u8)]
pub enum OperatingMode {
    InterruptOnTerminalCount = 0b000,
    HardwareRetriggerableOneShot = 0b001,
    RateGenerator = 0b010,
    SquareWaveGenerator = 0b011,
    SoftwareTriggeredStrobe = 0b100,
    HardwareTriggeredStrobe = 0b101,
}

#[repr(u8)]
pub enum AccessByte {
    // No latch count value command support.
    Low = 0b01,
    High = 0b10,
    LowAndHigh = 0b11,
}

#[repr(u8)]
pub enum Channel {
    Channel0 = 0b00,
    Channel1 = 0b01,
    Channel2 = 0b10,
    // No read-back command support.
}

pub struct Command {
    value: u8,
}

impl Command {
    pub fn new(operating_mode: OperatingMode, access_mode: AccessByte, channel: Channel) -> Self {
        Self {
            value: ((channel as u8) << 6)
                | ((access_mode as u8) << 4)
                | ((operating_mode as u8) << 1),
        }
    }

    pub fn set_operating_mode(&mut self, operating_mode: OperatingMode) {
        self.value.set_bits(1..4, operating_mode as u8);
    }

    pub fn set_access_mode(&mut self, access_mode: AccessByte) {
        self.value.set_bits(4..6, access_mode as u8);
    }

    pub fn set_channel(&mut self, channel: Channel) {
        self.value.set_bits(6..8, channel as u8);
    }

    pub fn as_u8(&self) -> u8 {
        self.value
    }
}

pub fn send_command(command: Command) {
    unsafe { ioports::WriteOnlyPort::<u8>::new(0x43) }.write(command.as_u8());
}

/// Sets the frequency of the programmable interrupt timer.
///
/// ## Safety
///
/// Setting an incorrect frequency or operating mode is undefined behaviour.
pub unsafe fn set_timer_freq(frequency: u32, operating_mode: OperatingMode) {
    assert!(
        frequency > TICK_RATE,
        "PIT frequency cannot be greater than {TICK_RATE}Hz"
    );

    send_command(Command::new(
        operating_mode,
        AccessByte::LowAndHigh,
        Channel::Channel0,
    ));
    let divisor = TICK_RATE / frequency;

    let mut data = unsafe { ioports::WriteOnlyPort::<u8>::new(0x40) };
    data.write(divisor as u8);
    data.write((divisor >> 8) as u8);
}
