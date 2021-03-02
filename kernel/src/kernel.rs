#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, abi_x86_interrupt, once_cell)]

#[macro_use]
extern crate log;
extern crate alloc;
extern crate libkernel;

mod drivers;
mod logging;
mod pic8259;
mod timer;

use core::ffi::c_void;
use libkernel::{BootInfo, ConfigTableEntry};

extern "C" {
    static _text_start: c_void;
    static _text_end: c_void;

    static _rodata_start: c_void;
    static _rodata_end: c_void;

    static _data_start: c_void;
    static _data_end: c_void;

    static _bss_start: c_void;
    static _bss_end: c_void;
}

#[cfg(debug_assertions)]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Trace
}

#[cfg(not(debug_assertions))]
fn get_log_level() -> log::LevelFilter {
    log::LevelFilter::Info
}

static mut SERIAL_OUT: drivers::io::Serial = drivers::io::Serial::new(drivers::io::COM1);

#[no_mangle]
#[export_name = "_start"]
extern "efiapi" fn kernel_main(
    boot_info: BootInfo<libkernel::memory::UEFIMemoryDescriptor, ConfigTableEntry>,
) -> ! {
    unsafe {
        SERIAL_OUT.init(drivers::io::SerialSpeed::S115200);
        drivers::io::set_stdout(&mut SERIAL_OUT);
    }

    match crate::logging::init_logger(crate::logging::LoggingModes::STDOUT, get_log_level()) {
        Ok(()) => {
            info!("Successfully loaded into kernel, with logging enabled.");
            debug!("Minimum logging level configured as: {:?}", get_log_level());
        }
        Err(error) => panic!("{}", error),
    }

    info!("Validating magic of BootInfo.");
    boot_info.validate_magic();

    debug!(
        "Detected CPU features: {:?}",
        libkernel::instructions::cpu_features()
    );

    unsafe { libkernel::instructions::init_segment_registers(0x0) };
    debug!("Zeroed segment registers.");

    libkernel::structures::gdt::init();
    info!("Successfully initialized GDT.");
    libkernel::structures::idt::init();
    info!("Successfully initialized IDT.");

    // `boot_info` will not be usable after initalizing the global allocator,
    //   due to the stack being moved in virtual memory.
    let framebuffer_pointer = boot_info.framebuffer_pointer().unwrap().clone();
    init_memory(boot_info);

    init_apic();

    info!("Initializing framebuffer driver.");
    let mut framebuffer_driver = drivers::graphics::framebuffer::FramebufferDriver::init(
        framebuffer_pointer.addr(),
        framebuffer_pointer.size(),
        framebuffer_pointer.stride(),
    );

    info!("Testing framebuffer driver.");
    for x in 0..640 {
        framebuffer_driver.write_pixel((x, 1), drivers::graphics::color::Color8i::new(20, 190, 20));
    }

    for x in 0..640 {
        framebuffer_driver.write_pixel((x, 2), drivers::graphics::color::Color8i::new(20, 190, 20));
    }

    // framebuffer_driver.LOG();

    framebuffer_driver.flush_pixels();

    info!("Kernel has reached safe shutdown state.");
    unsafe { libkernel::instructions::pwm::qemu_shutdown() }
}

fn init_memory(boot_info: BootInfo<libkernel::memory::UEFIMemoryDescriptor, ConfigTableEntry>) {
    use libkernel::memory::{global_memory, FrameState};

    info!("Initializing global memory.");
    unsafe { libkernel::memory::init_global_memory(boot_info.memory_map()) };

    debug!("Reserving frames from relevant UEFI memory descriptors.");

    use core::{lazy::OnceCell, ops::Range};
    let stack_frames = OnceCell::<Range<libkernel::memory::Frame>>::new();

    let mut last_frame_end = 0;
    for descriptor in boot_info.memory_map() {
        let cur_frame_start = (descriptor.phys_start.as_u64() / 0x1000) as usize;
        let new_frame_end = cur_frame_start + (descriptor.page_count as usize);

        // Checks for 'holes' in system memory which we shouldn't try to allocate to.
        if last_frame_end < cur_frame_start {
            unsafe {
                global_memory()
                    .acquire_frames(last_frame_end..cur_frame_start, FrameState::NonUsable)
                    .unwrap()
            };
        }

        // Reserve descriptor properly, and acquire stack frames if applicable.
        if descriptor.should_reserve() {
            let frame_range = cur_frame_start..new_frame_end;

            if descriptor.is_stack_descriptor() {
                debug!("Identified stack frames: {:?}", frame_range);
                let descriptor_stack_frames = unsafe {
                    global_memory()
                        .acquire_frames(frame_range, FrameState::Reserved)
                        .unwrap()
                };

                stack_frames
                    .set(descriptor_stack_frames)
                    .expect("multiple stack descriptors found");
            } else {
                unsafe {
                    global_memory()
                        .acquire_frames(frame_range, FrameState::Reserved)
                        .unwrap()
                };
            }
        }

        last_frame_end = new_frame_end;
    }

    info!("Initializing global allocator.");
    unsafe { libkernel::memory::GLOBAL_ALLOCATOR.init(stack_frames.get().unwrap().clone()) };

    info!("Global memory & the kernel global allocator have been initialized.");
}

fn init_apic() {
    crate::pic8259::enable();
    info!("Successfully initialized PIC.");
    info!("Configuring PIT frequency to 1000Hz.");
    crate::pic8259::set_timer_freq(crate::timer::TIMER_FREQUENCY as u32);

    debug!("Setting timer interrupt handler and enabling interrupts.");
    libkernel::structures::idt::set_interrupt_handler(32, crate::timer::tick_handler);
    libkernel::instructions::interrupts::enable();

    libkernel::structures::apic::local::load();
    let lapic = libkernel::structures::apic::local::local_apic_mut().unwrap();

    unsafe {
        debug!("Resetting and enabling local APIC (it may have already been enabled).");
        lapic.reset();
        lapic.enable();
        let timer = timer::Timer::new(crate::timer::TIMER_FREQUENCY / 1000);
        lapic.configure_spurious(u8::MAX, true);
        lapic.configure_timer(48, || timer.wait())
    }

    debug!("Disabling 8259 emulated PIC.");
    libkernel::instructions::interrupts::without_interrupts(|| unsafe {
        crate::pic8259::disable()
    });
    debug!("Updating IDT timer interrupt entry to local APIC-enabled function.");
    libkernel::structures::idt::set_interrupt_handler(48, timer::apic_timer_handler);
    debug!("Unmasking local APIC timer interrupt (it will fire now!).");
    lapic.timer().set_masked(false);

    info!("Core-local APIC configured and enabled (8259 PIC disabled).");
}
