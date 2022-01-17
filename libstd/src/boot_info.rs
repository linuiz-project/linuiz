use crate::{FFIOption, FramebufferInfo};

#[repr(C)]
pub struct BootInfo<MM, CTE> {
    memory_map_ptr: *const MM,
    memory_map_len: usize,
    config_table_ptr: *const CTE,
    config_table_len: usize,
    magic: u32,
    framebuffer: FFIOption<FramebufferInfo>,
}

impl<MM, CTE> BootInfo<MM, CTE> {
    const MAGIC: u32 = 0xAABB11FF;

    pub fn new(
        memory_map: &[MM],
        config_table: &[CTE],
        framebuffer: Option<FramebufferInfo>,
    ) -> Self {
        Self {
            memory_map_ptr: memory_map.as_ptr(),
            memory_map_len: memory_map.len(),
            config_table_ptr: config_table.as_ptr(),
            config_table_len: config_table.len(),
            magic: Self::MAGIC,
            framebuffer: match framebuffer {
                Some(some) => FFIOption::Some(some),
                None => FFIOption::None,
            },
        }
    }

    pub fn memory_map(&self) -> &[MM] {
        unsafe {
            core::ptr::slice_from_raw_parts(self.memory_map_ptr, self.memory_map_len)
                .as_ref()
                .unwrap()
        }
    }

    pub fn config_table(&self) -> &[CTE] {
        unsafe {
            core::ptr::slice_from_raw_parts(self.config_table_ptr, self.config_table_len)
                .as_ref()
                .unwrap()
        }
    }

    pub fn framebuffer_pointer(&self) -> Option<FramebufferInfo> {
        self.framebuffer.into()
    }

    pub fn validate_magic(&self) {
        assert_eq!(
            self.magic,
            Self::MAGIC,
            "boot_info is unaligned, or magic is otherwise corrupted"
        );
    }
}
