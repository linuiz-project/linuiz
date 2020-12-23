///! Graphics driver utilizing the EFI_GRAPHICS_OUTPUT_PROTOCOL to write to framebuffer.
use crate::drivers::graphics::color::{Color8i, Colors};
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

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        let dimensions = self.dimensions();
        if xy.0 >= dimensions.width || xy.1 >= dimensions.height {
            panic!("given coordinates are outside framebuffer");
        } else {
            unsafe {
                let index = (xy.0 + (xy.1 * dimensions.width)) as isize;
                self.backbuffer().offset(index).write_volatile(color);
            }
        }
    }

    pub fn clear(&mut self, color: Color8i, flush: bool) {
        let length = self.length();
        let backbuffer = self.backbuffer();
        unsafe {
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
        if self.has_backbuffer() {
            let length = self.length();
            let backbuffer = self.backbuffer;
            let framebuffer = self.framebuffer;

            unsafe {
                core::ptr::copy_nonoverlapping(backbuffer, framebuffer, length);
            }

            // clear the backbuffer
            self.clear(Colors::Black.into(), false)
        }
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

    fn backbuffer(&self) -> *mut Color8i {
        if self.has_backbuffer() {
            self.backbuffer
        } else {
            self.framebuffer
        }
    }

    fn has_backbuffer(&self) -> bool {
        self.backbuffer != core::ptr::null_mut::<Color8i>()
    }
}

impl core::fmt::Debug for FramebufferDriver {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("ProtocolGraphics")
            .field("Framebuffer", &self.framebuffer)
            .field("Backbuffer", &self.backbuffer)
            .finish()
    }
}
