use crate::memory::{align_down, aligned_slices};
///! Graphics driver utilizing the EFI_GRAPHICS_OUTPUT_PROTOCOL to write to framebuffer.
use core::mem::size_of;
use uefi::{
    prelude::BootServices,
    proto::console::gop::{GraphicsOutput, Mode},
    table::boot::MemoryType,
    ResultExt,
};

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

impl From<u32> for Color8i {
    fn from(from: u32) -> Self {
        let r = ((from >> 24) & 0xFF) as u8;
        let g = ((from >> 16) & 0xFF) as u8;
        let b = ((from >> 8) & 0xFF) as u8;
        let reserved = (from & 0xFF) as u8;

        Self { r, g, b, reserved }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Color {
    Black,
    Gray,
}

impl Into<Color8i> for Color {
    fn into(self) -> Color8i {
        match self {
            Color::Black => Color8i::new(0, 0, 0),
            Color::Gray => Color8i::new(100, 100, 100),
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct Size {
    width: usize,
    height: usize,
}

impl Size {
    fn new(width: usize, height: usize) -> Self {
        Size { width, height }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct ProtocolGraphics {
    // TODO(?) lock on these fields
    pub framebuffer: *mut Color8i,
    backbuffer: *mut Color8i,
    dimensions: Size,
}

impl ProtocolGraphics {
    pub fn new(boot_services: &BootServices, graphics_output: &mut GraphicsOutput) -> Self {
        // TODO(?) add some sensible way to choose the output mode
        let framebuffer = graphics_output.frame_buffer().as_mut_ptr() as *mut Color8i;
        let resolution = select_graphics_mode(graphics_output).info().resolution();
        let dimensions = Size::new(resolution.0, resolution.1);
        let byte_length = dimensions.width * dimensions.height * size_of::<Color8i>();

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

    // TODO this is technically unsafe since you can modify from multiple threads (?)
    pub fn write_pixel(&self, size: Size, color: Color8i) {
        let dimensions = self.dimensions();
        if size.width >= dimensions.width || size.height >= dimensions.height {
            panic!("given coordinates are outside framebuffer");
        } else {
            unsafe {
                let index = (size.width + (size.height * dimensions.width)) as isize;
                self.backbuffer.offset(index).write_volatile(color);
            }
        }
    }

    pub fn clear(&mut self, color: Color8i, flush: bool) {
        unsafe {
            let length = self.length();
            let backbuffer = self.backbuffer;
            for index in 0..length {
                backbuffer.offset(index as isize).write_volatile(color);
            }
        }

        if flush {
            self.flush_pixels();
        }
    }

    /// copy backbuffer to frontbuffer and zero backbuffer
    pub fn flush_pixels(&mut self) {
        let length = self.length();
        let backbuffer = self.backbuffer;
        let framebuffer = self.framebuffer;

        unsafe {
            core::ptr::copy(backbuffer, framebuffer, length);
        }

        // clear the backbuffer
        self.clear(Color::Black.into(), false)
    }

    pub fn dimensions(&self) -> Size {
        self.dimensions
    }

    pub fn length(&self) -> usize {
        let dimensions = self.dimensions();
        dimensions.width * dimensions.height
    }

    pub fn byte_length(&self) -> usize {
        self.length() * size_of::<Color8i>()
    }
}

impl core::fmt::Debug for ProtocolGraphics {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        formatter
            .debug_struct("ProtocolGraphics")
            .field("Framebuffer", &self.framebuffer)
            .field("Backbuffer", &self.backbuffer)
            .finish()
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
