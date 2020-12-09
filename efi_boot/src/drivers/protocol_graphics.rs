///! Graphics driver utilizing the EFI_GRAPHICS_OUTPUT_PROTOCOL to write to framebuffer.
use crate::memory::{align_down, aligned_slices};
use uefi::{
    prelude::BootServices,
    proto::console::gop::{BltPixel, GraphicsOutput, Mode, ModeInfo},
    table::boot::MemoryType,
    ResultExt,
};

#[repr(C)]
#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Color8i {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

pub struct ProtocolGraphics<'boot> {
    // TODO lock on these fields
    graphics_output: &'boot mut GraphicsOutput<'boot>,
    backbuffer: *mut Color8i,
}

impl<'boot> ProtocolGraphics<'boot> {
    pub fn new(
        boot_services: &BootServices,
        graphics_output: &'boot mut GraphicsOutput<'boot>,
    ) -> Self {
        // TODO(?) add some sensible way to choose the output mode
        let mode = select_graphics_mode(graphics_output);
        let mode_info = mode.info();

        // get framebuffer size
        let framebuffer_resolution = mode_info.resolution();
        let framebuffer_size =
            framebuffer_resolution.0 * framebuffer_resolution.1 * core::mem::size_of::<Color8i>();
        // allocate pages for backbuffer
        let backbuffer = if let Ok(completion) =
            boot_services.allocate_pool(MemoryType::LOADER_DATA, framebuffer_size)
        {
            completion.unwrap() as *mut Color8i
        } else {
            panic!("not enough memory to allocate backbuffer")
        };

        ProtocolGraphics {
            graphics_output,
            backbuffer,
        }
    }

    // this is technically unsafe since you can modify from multiple threads (?)
    pub fn write_pixel(&self, xy: (usize, usize), color: Color8i) {
        let dimensions = self.graphics_output.current_mode_info().resolution();
        if xy.0 < 0 || xy.0 >= dimensions.0 || xy.1 < 0 || xy.1 >= dimensions.1 {
            panic!("given coordinates are outside framebuffer");
        } else {
            unsafe {
                let index = (xy.0 + (xy.1 * dimensions.0)) as isize;
                self.backbuffer.offset(index).write_volatile(color);
            }
        }
    }

    pub fn flush_buffer() {}
}

fn select_graphics_mode(graphics_output: &mut GraphicsOutput) -> Mode {
    let graphics_mode = graphics_output
        .modes()
        .map(|mode| mode.expect("warning encountered while querying mode"))
        .last() // just select the largest resolution
        .unwrap();

    graphics_output
        .set_mode(&graphics_mode)
        .expect_success("failed to set graphics mode");

    graphics_mode
}
