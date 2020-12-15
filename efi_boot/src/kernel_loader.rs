///! Used to load a kernel file into memory, and return the kernel's entry point as a memory address.
use crate::{
    elf::{
        program_header::{ProgramHeader, ProgramHeaderType},
        section_header::SectionHeader,
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

pub const KERNEL_VADDRESS: usize = 0xFFFFFFFF80000000; // -2GB, page-aligned

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
    let program_header_buffer = &mut [0u8; size_of::<ProgramHeader>()];
    let mut current_disk_offset = kernel_header.program_headers_offset();

    for index in 0..kernel_header.program_header_count() {
        read_file(
            kernel_file,
            current_disk_offset as u64,
            program_header_buffer,
        );
        let program_header = ProgramHeader::parse(program_header_buffer)
            .expect("failed to parse program header from buffer");

        match program_header.ph_type() {
            ProgramHeaderType::PT_LOAD => {
                debug!(
                    "Identified loadable segment (index {}, disk offset {}): {:?}",
                    index, current_disk_offset, program_header
                );

                // calculate required variables for correctly loading segment into memory
                let aligned_address = align_down(
                    program_header.physical_address(),
                    program_header.alignment(),
                );
                // this is the offset within the page that the segment starts
                let page_offset = wrapping_sub(program_header.physical_address(), aligned_address);
                // size of the segment size + offset within the page
                let aligned_size = page_offset + program_header.memory_size();
                let pages_count = aligned_slices(aligned_size, program_header.alignment());

                debug!(
                    "Loading segment (index {}):\n Unaligned Address: {}\n Aligned Address: {}\n Unaligned Size: {}\n Aligned Size: {}",
                    index, program_header.physical_address(), aligned_address, program_header.memory_size(), aligned_size
                );

                // allocate pages for header
                let segment_page_buffer = allocate_pages(
                    boot_services,
                    // we take an address relative to kernel insertion
                    // point, but that doesn't really matter to the code
                    // in this context
                    AllocateType::Address(aligned_address),
                    MemoryType::LOADER_CODE,
                    pages_count,
                )
                // we won't ever explicitly deallocate this, so we only
                // care about the buffer (pointer is used to deallocate, usually)
                .buffer;

                // the segments won't always be aligned to pages, so take the slice of the buffer
                // that is equal to the program segment's lowaddr..highaddr

                let slice_end_index = page_offset + program_header.disk_size();
                let segment_slice = &mut segment_page_buffer[page_offset..slice_end_index];
                read_file(kernel_file, program_header.offset() as u64, segment_slice);

                if program_header.memory_size() > program_header.disk_size() {
                    // in this case, we need to zero-out the remaining memory so the segment
                    // doesn't point to garbage data (since we won't be reading anything valid into it)
                    let memory_end_index = page_offset + program_header.memory_size();
                    debug!(
                        "Zeroing segment section (index {}): from {} to {}",
                        index, slice_end_index, memory_end_index
                    );

                    for index in slice_end_index..memory_end_index {
                        segment_page_buffer[index] = 0x0;
                    }
                }

                debug!("Segment loaded (index {}).", index);
            }
            _ => {}
        }

        current_disk_offset += kernel_header.program_header_size() as usize;
    }
}

fn determine_section_bounds(
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) -> (Option<usize>, Option<usize>) {
    // REMARK (TODO?): it seems inefficient to read the section headers twice.
    //      the overhead is probably neglible, but it's something to keep in mind.

    // prepare variables
    let mut low_address: Option<usize> = None;
    let mut high_address: Option<usize> = None;
    let section_header_buffer = &mut [0u8; size_of::<SectionHeader>()];

    let mut section_disk_offset = kernel_header.section_headers_offset();
    for index in 0..kernel_header.section_header_count() {
        // set position in file and read section header into memory
        read_file(
            kernel_file,
            section_disk_offset as u64,
            section_header_buffer,
        );
        let section_header = SectionHeader::parse(section_header_buffer)
            .expect("failed to read section header from buffer");

        debug!(
            "Determining address space of section (index {}): {:?}",
            index, section_header
        );

        // use exclusive if to ensure we are able to increment disk offset at end of for
        if section_header.address() > 0x0 {
            // high address is the highest possible address the section overlaps
            let section_high_address = section_header.address() + section_header.entry_size();
            debug!(
                "Determining section address space (index {}): low {}, high {}",
                index,
                section_header.address(),
                section_high_address
            );

            if low_address.is_none() || section_header.address() < low_address.unwrap() {
                low_address = Some(section_header.address());
            }

            if high_address.is_none() || section_high_address > high_address.unwrap() {
                high_address = Some(section_high_address);
            }
        }

        section_disk_offset += kernel_header.section_header_size() as usize;
    }

    (low_address, high_address)
}

fn allocate_sections(
    boot_services: &BootServices,
    kernel_file: &mut RegularFile,
    kernel_header: &ELFHeader64,
) {
    // this will help determine where and how many pages we need to allocate for section entries
    let (low_address_option, high_address_option) =
        determine_section_bounds(kernel_file, kernel_header);

    if low_address_option.is_none() || high_address_option.is_none() {
        panic!(
            "Address space for section entires is invalid: low {:?}, high {:?}",
            low_address_option, high_address_option
        );
    }

    let low_address = low_address_option.unwrap();
    let high_address = high_address_option.unwrap();
    let section_buffer_size = high_address - low_address;
    debug!(
        "Determined section entry address space:\n Low Address {}\n High Address {}\n Address Space {}",
        low_address, high_address, section_buffer_size
    );

    if section_buffer_size == 0x0 {
        return; // no section entries to load
    }

    // get data relative to a low address that is aligned on page boundries
    let aligned_low_address = align_down(low_address, PAGE_SIZE);
    let aligned_section_buffer_size = high_address - aligned_low_address;
    // this offset tells us how far from index 0 we need to travel to get the true bottom of
    // the addressed section memory
    let pages_count = aligned_slices(aligned_section_buffer_size, PAGE_SIZE);

    debug!(
        "Allocating {} pages at address {} for section buffer.",
        pages_count, aligned_low_address
    );
    // allocate buffer for section entries
    let section_buffer = allocate_pages(
        boot_services,
        AllocateType::Address(aligned_low_address),
        MemoryType::LOADER_DATA,
        pages_count,
    )
    // we just want the buffer, we won't explicitly deallocate this
    .buffer;

    // just a container to hold current section header
    let section_header_buffer = &mut [0u8; size_of::<SectionHeader>()];
    let mut section_disk_offset = kernel_header.section_headers_offset();
    for index in 0..kernel_header.section_header_count() {
        read_file(
            kernel_file,
            section_disk_offset as u64,
            section_header_buffer,
        );
        let section_header = SectionHeader::parse(section_header_buffer)
            .expect("failed to read section header from buffer");

        // use exclusive if to ensure we are able to increment disk offset at end of for
        if section_header.entry_size() > 0 {
            debug!(
                "Identified section header for loading (index {}, disk offset {}): {:?}",
                index, section_disk_offset, section_header
            );

            // low address of the section relative to the allocated section buffer
            let relative_section_low_address = section_header.address() - aligned_low_address;
            let relative_section_high_address =
                relative_section_low_address + section_header.entry_size();
            // get slice of buffer representing section
            let section_slice =
                &mut section_buffer[relative_section_low_address..relative_section_high_address];

            read_file(kernel_file, section_header.offset() as u64, section_slice);
            debug!("Allocated memory pages for section header's entry.");
        }

        section_disk_offset += section_header.size();
    }
}
