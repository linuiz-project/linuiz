pub use x86_64::instructions::tables::load_tss;

use x86_64::structures::tss;

#[repr(C, packed(4))]
#[derive(Debug, Clone, Copy)]
pub struct TaskStateSegment(tss::TaskStateSegment);

// SAFETY: Zeroed memory is a valid state for a TSS.
unsafe impl bytemuck::Zeroable for TaskStateSegment {}

impl core::ops::Deref for TaskStateSegment {
    type Target = tss::TaskStateSegment;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for TaskStateSegment {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
