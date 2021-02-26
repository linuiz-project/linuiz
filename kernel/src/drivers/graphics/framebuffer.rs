#![allow(dead_code)]

use crate::drivers::graphics::color::{Color8i, Colors};
use libkernel::{memory::GLOBAL_ALLOCATOR, Size, VirtAddr};
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
            let alloc_to_ptr = GLOBAL_ALLOCATOR.alloc_to(mmio_frames) as *mut Color8i;

            core::slice::from_raw_parts_mut(alloc_to_ptr, pixel_len)
        };

        unsafe {
            let ptr = libkernel::alloc!(byte_len) as *mut Color8i;
            let backbuffer = unsafe { core::slice::from_raw_parts_mut(ptr, pixel_len) };

            info!(
                "BACKBUFFER {:?}: {}",
                ptr,
                GLOBAL_ALLOCATOR
                    .get_addressor()
                    .is_mapped(VirtAddr::from_ptr(ptr))
            );

            Self {
                framebuffer: RwLock::new(framebuffer),
                backbuffer: RwLock::new(backbuffer),
                dimensions,
            }
        }
    }

    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        let dimensions = self.dimensions();

        if xy.0 < dimensions.width() && xy.1 < dimensions.height() {
            let index = xy.0 + (xy.1 * dimensions.width());
            self.backbuffer.write()[index] = color;
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
        self.dimensions().len()
    }

    pub fn byte_len(&self) -> usize {
        self.pixel_len() * core::mem::size_of::<Color8i>()
    }
}
