use core::mem::size_of;
use libstd::elf::{ELFHeader64, SegmentHeader, SegmentType};
use uefi::proto::media::file::RegularFile;

pub fn allocate_segments(
    boot_services: &uefi::prelude::BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    // Define some function-statics for performance.
    let mut segment_header_buffer = [0u8; size_of::<SegmentHeader>()];
    let mut segment_header_disk_offset = kernel_header.segment_headers_offset() as u64;

    for _ in 0..kernel_header.segment_header_count() {
        // Read the segment header from disk into memory.
        let segment_header = unsafe {
            crate::read_file(
                kernel_file,
                segment_header_disk_offset,
                &mut segment_header_buffer,
            );

            (segment_header_buffer.as_ptr() as *const SegmentHeader)
                .as_ref()
                .unwrap()
        };

        // TODO: Also process GNU_RELRO segments.
        // Ensure the segment needs to be loaded at all.
        if segment_header.ty == SegmentType::LOAD {
            debug!("Identified loadable segment:\n{:#?}", segment_header);

            // Determine if this segment requires its own page mapping.
            // NOTE: This could be required due to an odd bug with rust.lld compilation?
            //       It doesn't seem like a normal issue. This, however, handles it so
            //       far as I know.
            let segment_buffer =
                if (segment_header.virt_addr.as_usize() % segment_header.align) == 0 {
                    debug!("Segment is self-aligned. Allocating buffer & slicing.");

                    // Align the address of the segment to page boundaries.
                    let page_aligned_addr =
                        libstd::align_down(segment_header.virt_addr.as_usize(), 0x1000);
                    let alignment_offset = segment_header.virt_addr.as_usize() - page_aligned_addr;
                    let aligned_size = alignment_offset + segment_header.mem_size;

                    let buffer = crate::allocate_pages(
                        boot_services,
                        uefi::table::boot::AllocateType::Address(page_aligned_addr),
                        crate::KERNEL_CODE,
                        // Determine how many pages the segment covers.
                        libstd::align_up_div(aligned_size, 0x1000),
                    );

                    // Handle any internal page offsets (i.e. segment has a non-page alignment).
                    &mut buffer[alignment_offset..(alignment_offset + segment_header.mem_size)]
                } else {
                    debug!("Segment is not self-aligned. Slicing memory directly.");

                    // If this segment's address doesn't align, simply create a slice over the region.
                    unsafe {
                        core::slice::from_raw_parts_mut(
                            segment_header.virt_addr.as_ptr::<u8>() as *mut _,
                            segment_header.mem_size,
                        )
                    }
                };

            // Read the file data into the buffer.
            crate::read_file(kernel_file, segment_header.offset as u64, segment_buffer);

            // Sometimes a segment contains extra space for data, and must be zeroed out.
            if segment_header.mem_size > segment_header.disk_size {
                segment_buffer[segment_header.disk_size..].fill(0x0);
            }
        }

        segment_header_disk_offset += size_of::<SegmentHeader>() as u64;
    }
}
