use alloc::boxed::Box;
use core::sync::atomic::AtomicU64;
use libstd::structures::apic::APIC;
use x86_64::structures::idt::InterruptDescriptorTable;

mod handlers;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum InterruptVector {
    Timer = 32,
    CMCI,
    Performance,
    ThermalSensor,
    LINT0,
    LINT1,
    Error,
    Storage,
    // APIC spurious interrupt is default mapped to 255.
    Spurious = u8::MAX,
}

pub struct InterruptController {
    apic: APIC,
    idt: Box<InterruptDescriptorTable>,
    counters: [AtomicU64; 256],
}

impl InterruptController {
    const ATOMIC_ZERO: AtomicU64 = AtomicU64::new(0);

    pub fn create() -> Self {
        let mut idt = Box::new(InterruptDescriptorTable::new());
        libstd::structures::idt::configure(&mut idt);
        idt[InterruptVector::Spurious as usize].set_handler_fn(handlers::spurious_handler);
        unsafe { idt.load_unsafe() };

        use libstd::{
            instructions::interrupts,
            registers::MSR,
            structures::{apic::*, idt::InterruptStackFrame, pic8259},
        };

        extern "x86-interrupt" fn pit_tick_handler(_: InterruptStackFrame) {
            unsafe { MSR::IA32_GS_BASE.write_unchecked(MSR::IA32_GS_BASE.read_unchecked() + 1) };
            pic8259::end_of_interrupt(pic8259::InterruptOffset::Timer);
        }

        extern "x86-interrupt" fn apit_counter_deplete(_: InterruptStackFrame) {
            panic!("APIT has fired an interrupt during initialization (NOTE: APIT should not have enough time to fully deplete its counter register).");
        }

        interrupts::disable();

        debug!("Configuring APIC & APIT.");
        let apic = APIC::from_msr().expect("APIC has already been configured on this core");
        unsafe {
            apic.hw_enable();
            apic.reset();
            apic.sw_enable();
        }
        apic.write_register(Register::TimerDivisor, TimerDivisor::Div1 as u32);
        apic.timer().set_mode(TimerMode::OneShot);

        unsafe {
            debug!("Determining APIT frequency.");

            // Configure PIT & APIT interrupts.
            idt[pic8259::InterruptOffset::Timer as usize].set_handler_fn(pit_tick_handler);
            pic8259::enable(pic8259::InterruptLines::TIMER);
            pic8259::pit::set_timer_freq(1000, pic8259::pit::OperatingMode::RateGenerator);

            idt[254].set_handler_fn(apit_counter_deplete);
            apic.timer().set_vector(254);

            // Wait for PIT to tick 10 times, then stop APIT ticking.
            MSR::IA32_GS_BASE.write_unchecked(0);
            // Enable PIT counter.
            interrupts::enable();
            // 'Enable' the APIT to begin counting down in `Register::TimerCurrentCount`
            apic.write_register(Register::TimerInitialCount, u32::MAX);
            // Wait for 10ms to get good average tickrate.
            while MSR::IA32_GS_BASE.read_unchecked() < 10 {}
            // Disable PIT counter.
            interrupts::disable();
            MSR::IA32_GS_BASE.write_unchecked(0);

            // Un-configure the APIT & PIT interrupts.
            use x86_64::structures::idt::Entry;
            idt[pic8259::InterruptOffset::Timer as usize] = Entry::missing();
            pic8259::disable();

            idt[254] = Entry::missing();
            // APIT vector is set later in this function.
        }

        apic.write_register(
            Register::TimerInitialCount,
            (u32::MAX - apic.read_register(Register::TimerCurrentCount)) / 10,
        );
        debug!(
            "APIT frequency: {}KHz",
            apic.read_register(Register::TimerInitialCount)
        );

        // Configure interrupt handlers.
        idt[InterruptVector::Timer as usize].set_handler_fn(handlers::apit_handler);
        idt[InterruptVector::Error as usize].set_handler_fn(handlers::error_handler);

        // Configure timer.
        apic.timer().set_vector(InterruptVector::Timer as u8);
        apic.timer().set_mode(TimerMode::Periodic);
        apic.timer().set_masked(false);
        // Set default vectors.
        apic.cmci().set_vector(InterruptVector::CMCI as u8);
        apic.performance()
            .set_vector(InterruptVector::Performance as u8);
        apic.thermal_sensor()
            .set_vector(InterruptVector::ThermalSensor as u8);
        apic.lint0().set_vector(InterruptVector::LINT0 as u8);
        apic.lint1().set_vector(InterruptVector::LINT1 as u8);
        // Configure error register.
        apic.error().set_vector(InterruptVector::Error as u8);
        apic.error().set_masked(false);

        debug!("Core-local APIC configured and enabled.");

        Self {
            apic,
            idt,
            counters: [Self::ATOMIC_ZERO; 256],
        }
    }

    #[inline]
    pub unsafe fn sw_enable(&self) {
        self.apic.sw_enable();
    }

    #[inline]
    pub unsafe fn sw_disable(&self) {
        self.apic.sw_disable();
    }

    #[inline]
    pub fn interrupt_command(&self) -> libstd::structures::apic::icr::InterruptCommandRegister {
        self.apic.interrupt_command()
    }

    #[inline]
    pub(super) fn end_of_interrupt(&self) {
        self.apic.end_of_interrupt();
    }
}
