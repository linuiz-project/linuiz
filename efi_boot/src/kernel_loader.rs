///! Used to load a kernel file into memory, and return the kernel's entry point as a memory address.
use crate::{
    elf::{
        program_header::{ProgramHeader, ProgramHeaderType},
        ELFHeader64,
    },
    file::read_file,
    memory::{align_down, aligned_slices, allocate_pages, PAGE_SIZE},
};
use core::{intrinsics::wrapping_sub, mem::size_of};
use uefi::{
    prelude::BootServices,
    proto::media::file::RegularFile,
    table::boot::{AllocateType, MemoryType},
    ResultExt,
};

const KERNEL_CODE_TYPE: u32 = 0xFFFFFF00;

/// reads an ELF binary from the given file, and loads it into
/// memory, returning the entry address
pub fn load_kernel(boot_services: &BootServices, mut kernel_file: RegularFile) -> usize {
    let kernel_header = acquire_kernel_header(&mut kernel_file);
    info!("Kernel header read into memory.");
    debug!("{:?}", kernel_header);

    allocate_segments(boot_services, &mut kernel_file, &kernel_header);
    info!("Kernel successfully read into memory.");

    kernel_header.entry_address()
}

fn acquire_kernel_header(kernel_file: &mut RegularFile) -> ELFHeader64 {
    // allocate a block large enough to hold the header
    let mut kernel_header_buffer = [0u8; size_of::<ELFHeader64>()];

    // read the file into the buffer
    kernel_file
        .read(&mut kernel_header_buffer)
        .expect_success("failed to read kernel header into memory");
    let kernel_header =
        ELFHeader64::parse(&kernel_header_buffer).expect("failed to parse header from buffer");

    kernel_header
}

fn allocate_segments(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    let segment_header_buffer = &mut [0u8; size_of::<ProgramHeader>()];
    let mut segment_header_disk_offset = kernel_header.program_headers_offset();

    for index in 0..kernel_header.program_header_count() {
        read_file(
            kernel_file,
            segment_header_disk_offset as u64,
            segment_header_buffer,
        );
        let segment_header = ProgramHeader::parse(segment_header_buffer)
            .expect("failed to parse program header from buffer");

        if segment_header.ph_type() == ProgramHeaderType::PT_LOAD {
            debug!(
                "Identified loadable segment (index {}, disk offset {}): {:?}",
                index, segment_header_disk_offset, segment_header
            );

            // calculate required variables for correctly loading segment into memory
            let aligned_address = align_down(
                segment_header.physical_address(),
                segment_header.alignment(),
            );
            // this is the offset within the page that the segment starts
            let page_offset = wrapping_sub(segment_header.physical_address(), aligned_address);
            // size of the segment size + offset within the page
            let aligned_size = page_offset + segment_header.memory_size();
            let pages_count = aligned_slices(aligned_size, segment_header.alignment());

            debug!(
                    "Loading segment (index {}):\n Unaligned Address: {}\n Unaligned Size: {}\n Aligned Address: {}\n Aligned Size: {}\n End Address: {}\n Pages: {}",
                    index, segment_header.physical_address(), segment_header.memory_size(), aligned_address, aligned_size, aligned_address + (pages_count * PAGE_SIZE), pages_count
                );

            // allocate pages for header
            let segment_page_buffer = allocate_pages(
                boot_services,
                // we take an address relative to kernel insertion
                // point, but that doesn't really matter to the code
                // in this context
                AllocateType::Address(aligned_address),
                MemoryType::custom(KERNEL_CODE_TYPE),
                pages_count,
            )
            // we won't ever explicitly deallocate this, so we only
            // care about the buffer (pointer is used to deallocate, usually)
            .buffer;

            // the segments might not always be aligned to pages, so take the slice of the buffer
            // that is equal to the program segment's lowaddr..highaddr
            let slice_end_index = page_offset + segment_header.disk_size();
            let segment_slice = &mut segment_page_buffer[page_offset..slice_end_index];
            // finally, read the program segment into memory
            read_file(kernel_file, segment_header.offset() as u64, segment_slice);

            // sometimes a segment contains extra space for data, and must be zeroed out before any jumps
            if segment_header.memory_size() > segment_header.disk_size() {
                // in this case, we need to zero-out the remaining memory so the segment
                // doesn't point to garbage data (since we won't be reading anything valid into it)
                let memory_end_index = page_offset + segment_header.memory_size();
                debug!(
                    "Zeroing segment section (index {}): from {} to {}, total {}",
                    index,
                    slice_end_index,
                    memory_end_index,
                    memory_end_index - slice_end_index
                );

                for index in slice_end_index..memory_end_index {
                    segment_page_buffer[index] = 0x0;
                }
            }

            debug!("Segment loaded (index {}).", index);
        }

        // update the segment header offset so we can read next segment
        segment_header_disk_offset += kernel_header.program_header_size() as usize;
    }
}
