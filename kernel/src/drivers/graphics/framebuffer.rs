use crate::drivers::graphics::color::{Color8i, Colors};
///! Graphics driver utilizing the EFI_GRAPHICS_OUTPUT_PROTOCOL to write to framebuffer.
use core::mem::size_of;
use efi_boot::Size;

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct FramebufferDriver {
    // TODO(?) lock on these fields
    framebuffer: *mut Color8i,
    backbuffer: *mut Color8i,
    dimensions: Size,
}

impl FramebufferDriver {
    pub fn new(framebuffer: *mut Color8i, backbuffer: *mut Color8i, dimensions: Size) -> Self {
        FramebufferDriver {
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
        self.clear(Colors::Black.into(), false)
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

impl core::fmt::Debug for FramebufferDriver {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> Result<(), core::fmt::Error> {
        formatter
            .debug_struct("ProtocolGraphics")
            .field("Framebuffer", &self.framebuffer)
            .field("Backbuffer", &self.backbuffer)
            .finish()
    }
}
