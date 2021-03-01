#![allow(dead_code)]

use core::fmt::Write;

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
        let pixel_len = dimensions.len();
        let byte_len = pixel_len * core::mem::size_of::<Color8i>();

        let framebuffer = unsafe {
            let start_frame_index = (buffer_addr.as_u64() / 0x1000) as usize;
            let end_frame_index = start_frame_index + ((byte_len + 0xFFF) / 0x1000);
            let mmio_frames = libkernel::memory::global_memory()
                .acquire_frames(
                    start_frame_index..end_frame_index,
                    libkernel::memory::FrameState::MMIO,
                )
                .unwrap();

            core::slice::from_raw_parts_mut(libkernel::alloc_to!(mmio_frames), pixel_len)
        };

        Self {
            framebuffer: RwLock::new(framebuffer),
            backbuffer: RwLock::new(unsafe {
                core::slice::from_raw_parts_mut(libkernel::alloc!(byte_len), pixel_len)
            }),
            dimensions,
        }
    }

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        self.backbuffer.write()[self.point_to_index(xy)] = color;
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
        self.dimensions().len()
    }

    pub fn byte_len(&self) -> usize {
        self.pixel_len() * core::mem::size_of::<Color8i>()
    }

    fn point_to_index(&self, point: (usize, usize)) -> usize {
        (point.1 * self.dimensions().width()) + point.0
    }

    pub fn LOG(&self) {
        let so = unsafe { &mut crate::SERIAL_OUT };
        for color_x in 0..20 {
            for color_y in 0..20 {
                so.write_fmt(format_args!(
                    "{:?}",
                    self.backbuffer.read()[self.point_to_index((color_x, color_y))],
                ))
                .unwrap();
            }

            so.write_char('\n').unwrap();
        }
    }
}
