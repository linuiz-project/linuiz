use crate::{FFIOption, FramebufferPointer};

#[repr(C)]
pub struct BootInfo<MM> {
    memory_map_ptr: *const MM,
    memory_map_len: usize,
    magic: u32,
    framebuffer: FFIOption<FramebufferPointer>,
}

impl<MM> BootInfo<MM> {
    pub fn new(memory_map: &[MM], framebuffer: Option<FramebufferPointer>) -> Self {
        Self {
            memory_map_ptr: memory_map.as_ptr(),
            memory_map_len: memory_map.len(),
            magic: 0xAABB11FF,
            framebuffer: match framebuffer {
                Some(some) => FFIOption::Some(some),
                None => FFIOption::None,
            },
        }
    }

    pub fn memory_map(&self) -> &[MM] {
        unsafe { &*core::ptr::slice_from_raw_parts(self.memory_map_ptr, self.memory_map_len) }
    }

    pub fn framebuffer_pointer(&self) -> Option<FramebufferPointer> {
        self.framebuffer.into()
    }

    pub fn validate_magic(&self) {
        if self.magic != 0xAABB11FF {
            panic!("boot_info is unaligned, or magic is otherwise corrupted");
        }
    }
}
