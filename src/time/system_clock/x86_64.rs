enum ClockSource {
    Hpet,
    Rtc,
}

pub struct ArchClock {
    frequency: u64,
    source: ClockSource,
}

impl ArchClock {
    pub fn configure() -> Self {
        todo!()
    }
}
