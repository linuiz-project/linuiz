#![allow(dead_code)]

use crate::drivers::graphics::color::{Color8i, Colors};
use libkernel::Size;
use spin::RwLock;

#[repr(C)]
pub struct FramebufferDriver<'fbuf, 'bbuf> {
    framebuffer: RwLock<&'fbuf mut [Color8i]>,
    backbuffer: RwLock<&'bbuf mut [Color8i]>,
    dimensions: Size,
}

impl<'fbuf, 'bbuf> FramebufferDriver<'fbuf, 'bbuf> {
    pub fn init(buffer_addr: libkernel::PhysAddr, dimensions: Size) -> Self {
        let pixel_len = dimensions.width * dimensions.height;
        let byte_len = pixel_len * core::mem::size_of::<Color8i>();

        let framebuffer = unsafe {
            let frame_iter =
                libkernel::memory::Frame::range_count(buffer_addr, (byte_len + 0xFFF) / 0x1000);
            let alloc_to_ptr = libkernel::memory::alloc_to(frame_iter) as *mut Color8i;

            core::slice::from_raw_parts_mut(alloc_to_ptr, pixel_len)
        };

        let backbuffer = unsafe {
            core::slice::from_raw_parts_mut(
                alloc::alloc::alloc(core::alloc::Layout::from_size_align(byte_len, 16).unwrap())
                    as *mut Color8i,
                pixel_len,
            )
        };

        Self {
            framebuffer: RwLock::new(framebuffer),
            backbuffer: RwLock::new(backbuffer),
            dimensions,
        }
    }

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        let dimensions = self.dimensions();

        if xy.0 < dimensions.width && xy.1 < dimensions.height {
            self.backbuffer.write()[xy.0 + (xy.1 * dimensions.width)] = color;
        } else {
            panic!("given coordinates are outside framebuffer");
        }
    }

    pub fn clear(&mut self, color: Color8i) {
        self.backbuffer.write().fill(color);
    }

    /// Copy backbuffer to frontbuffer and zero backbuffer
    pub fn flush_pixels(&mut self) {
        {
            let mut framebuffer = self.framebuffer.write();
            let backbuffer = self.backbuffer.read();

            framebuffer.copy_from_slice(*backbuffer);
        }

        self.clear(Colors::Black.into());
    }

    pub fn dimensions(&self) -> Size {
        self.dimensions
    }

    pub fn pixel_len(&self) -> usize {
        let dimensions = self.dimensions();
        dimensions.width * dimensions.height
    }

    pub fn byte_len(&self) -> usize {
        self.pixel_len() * core::mem::size_of::<Color8i>()
    }
}
