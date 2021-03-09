use core::u8;

#[derive(Debug)]
pub struct PCIEDeviceHeader {
    vendor_id: u16,
    device_id: u16,
    command: u16,
    status: u16,
    revision_id: u8,
    prog_interface: u8,
    subclass: u8,
    class: u8,
    cache_line_size: u8,
    latency_timer: u8,
    header_type: u8,
    bist: u8,
}

pub struct PCIEDeviceIterator {
    base_addr: x86_64::PhysAddr,
    start_bus: u8,
    end_bus: u8,
    cur_bus: u8
}

impl Iterator for PCIEDeviceIterator {
    type Item = PCIEBus;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cur_bus < self.end_bus {
            let offset_addr = self.base_addr + ((bus_index as u64) << 20);
            let frame_index = (offset_addr.as_u64() / 0x1000) as usize;
            let mmio_frames = unsafe {
                crate::memory::global_memory()
                    .acquire_frame(frame_index, crate::memory::FrameState::MMIO)
                    .unwrap()
                    .into_iter()
            };

            let mmio = crate::memory::mmio::unmapped_mmio(mmio_frames)
                .unwrap()
                .map();
        } else {
            None
        }
    }
}