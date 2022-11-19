#![allow(dead_code)]

use crate::drivers::graphics::color::{Color8i, Colors};
use lzstd::{Address, Physical, Size};
use spin::{Mutex, RwLock};

#[repr(C)]
pub struct FramebufferDriver {
    framebuffer: Mutex<*mut Color8i>,
    backbuffer: RwLock<*mut Color8i>,
    dimensions: Size,
    scanline_width: usize,
}

impl FramebufferDriver {
    pub fn new(buffer_addr: Address<Physical>, dimensions: Size, scanline_width: usize) -> Self {
        let pixel_len = scanline_width * dimensions.height();
        let byte_len = pixel_len * core::mem::size_of::<Color8i>();

        let framebuffer = unsafe {
            lzstd::memory::malloc::get()
                .alloc_against(buffer_addr.frame_index(), (byte_len + 0xFFF) / 0x1000)
                .expect("Allocation error occurred when attempting to create pixelbuffer")
                .cast()
                .expect("Allocated region is of invalid alignment for Color8i")
                .into_parts()
                .0
        };

        let backbuffer = unsafe {
            lzstd::memory::malloc::get()
                .alloc(
                    byte_len,
                    core::num::NonZeroUsize::new(core::mem::align_of::<Color8i>()),
                )
                .expect("Allocation error occurred when attempting to create pixelbuffer")
                .cast()
                .expect("Allocated region is of invalid alignment for Color8i")
                .into_parts()
                .0
        };

        info!("{:?} {}", dimensions, scanline_width);

        Self {
            framebuffer: Mutex::new(framebuffer),
            backbuffer: RwLock::new(backbuffer),
            dimensions,
            scanline_width,
        }
    }

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        if self.contains_point(xy) {
            unsafe {
                self.backbuffer
                    .write()
                    .add(self.point_to_offset(xy))
                    .write_volatile(color)
            };
        } else {
            panic!("point lies without framebuffer");
        }
    }

    pub fn clear(&mut self, color: Color8i) {
        let backbuffer = self.backbuffer.write();
        for y in 0..self.dimensions().height() {
            for x in 0..self.dimensions().width() {
                unsafe {
                    backbuffer
                        .add(self.point_to_offset((x, y)))
                        .write_volatile(color)
                }
            }
        }
    }

    /// Copy backbuffer to frontbuffer and zero backbuffer
    pub fn flush_pixels(&mut self) {
        unsafe {
            core::ptr::copy_nonoverlapping(
                *self.backbuffer.read(),
                *self.framebuffer.lock(),
                self.dimensions().len(),
            )
        };

        self.clear(Colors::Black.into());
    }

    pub const fn dimensions(&self) -> Size {
        self.dimensions
    }

    const fn point_to_offset(&self, point: (usize, usize)) -> usize {
        (point.1 * self.scanline_width) + point.0
    }

    const fn contains_point(&self, point: (usize, usize)) -> bool {
        point.0 < self.dimensions().width() && point.1 < self.dimensions().height()
    }
}
