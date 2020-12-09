use crate::memory::{align_down, aligned_slices};
///! Graphics driver utilizing the EFI_GRAPHICS_OUTPUT_PROTOCOL to write to framebuffer.
use core::mem::size_of;
use uefi::{
    prelude::BootServices,
    proto::console::gop::{GraphicsOutput, Mode},
    table::boot::MemoryType,
    ResultExt,
};

pub const COLOR_BLACK: Color8i = Color8i::new(0, 0, 0);
pub const COLOR_GRAY: Color8i = Color8i::new(100, 100, 100);

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Color8i {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    reserved: u8,
}

impl Color8i {
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Color8i {
            r,
            g,
            b,
            reserved: 0x0,
        }
    }
}

pub struct ProtocolGraphics {
    // TODO(?) lock on these fields
    framebuffer: *mut Color8i,
    backbuffer: *mut Color8i,
    dimensions: (usize, usize),
}

impl ProtocolGraphics {
    pub fn new(boot_services: &BootServices, graphics_output: &mut GraphicsOutput) -> Self {
        // TODO(?) add some sensible way to choose the output mode
        let framebuffer = graphics_output.frame_buffer().as_mut_ptr() as *mut Color8i;
        let dimensions = select_graphics_mode(graphics_output).info().resolution();
        let byte_length = dimensions.0 * dimensions.1 * size_of::<Color8i>();

        // allocate pages for backbuffer
        let backbuffer = if let Ok(completion) =
            boot_services.allocate_pool(MemoryType::LOADER_DATA, byte_length)
        {
            completion.unwrap() as *mut Color8i
        } else {
            panic!("not enough memory to allocate backbuffer")
        };

        ProtocolGraphics {
            framebuffer,
            backbuffer,
            dimensions,
        }
    }

    // this is technically unsafe since you can modify from multiple threads (?)
    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        let dimensions = self.dimensions();
        if xy.0 >= dimensions.0 || xy.1 >= dimensions.1 {
            panic!("given coordinates are outside framebuffer");
        } else {
            unsafe {
                let index = (xy.0 + (xy.1 * dimensions.0)) as isize;
                self.backbuffer.offset(index).write_volatile(color);
            }
        }
    }

    pub fn clear(&mut self, color: Color8i, flush: bool) {
        unsafe {
            let length = self.length();
            let framebuffer = self.framebuffer;
            for index in 0..length {
                framebuffer.offset(index as isize).write_volatile(color);
            }
        }

        if flush {
            self.flush_pixels();
        }
    }

    /// copy backbuffer to frontbuffer and zero backbuffer
    pub fn flush_pixels(&mut self) {
        unsafe {
            let length = self.length();
            let backbuffer = self.backbuffer;
            let framebuffer = self.framebuffer;
            core::ptr::copy(backbuffer, framebuffer, length);

            for index in 0..length {
                backbuffer
                    .offset(index as isize)
                    .write_volatile(COLOR_BLACK);
            }
        }
    }

    pub fn dimensions(&self) -> (usize, usize) {
        self.dimensions
    }

    pub fn length(&self) -> usize {
        let dimensions = self.dimensions();
        dimensions.0 * dimensions.1
    }

    pub fn byte_length(&self) -> usize {
        self.length() * size_of::<Color8i>()
    }
}

fn select_graphics_mode(graphics_output: &mut GraphicsOutput) -> Mode {
    let graphics_mode = graphics_output
        .modes()
        .map(|mode| mode.expect("warning encountered while querying mode"))
        .find(|ref mode| {
            let info = mode.info();
            info.resolution() == (1024, 768)
        })
        .unwrap();

    graphics_output
        .set_mode(&graphics_mode)
        .expect_success("failed to set graphics mode");

    graphics_mode
}
