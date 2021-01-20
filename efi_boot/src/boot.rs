#![no_std]
#![no_main]
#![feature(abi_efiapi)]
#![feature(const_option)]
#![feature(negative_impls)]
#![feature(core_intrinsics)]
#![feature(unsafe_cell_get_mut)]

#[macro_use]
extern crate log;
extern crate rlibc;

use core::{
    cell::UnsafeCell,
    intrinsics::wrapping_sub,
    mem::{size_of, transmute},
    ptr::slice_from_raw_parts_mut,
};
use efi_boot::{
    align_down,
    elf::{
        program_header::{ProgramHeader, ProgramHeaderType},
        ELFHeader64,
    },
    memory::PAGE_SIZE,
    FramebufferPointer,
};
use uefi::{
    prelude::BootServices,
    proto::{
        console::gop::{GraphicsOutput, Mode},
        loaded_image::{DevicePath, LoadedImage},
        media::{
            file::{Directory, File, FileAttribute, FileMode, RegularFile},
            fs::SimpleFileSystem,
        },
        Protocol,
    },
    table::{
        boot::{AllocateType, MemoryDescriptor, MemoryType},
        runtime::ResetType,
        Boot, SystemTable,
    },
    Handle, ResultExt, Status,
};
use uefi_macros::entry;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");
const MINIMUM_MEMORY: usize = 0xF424000; // 256MB
const KERNEL_CODE: MemoryType = MemoryType::custom(0xFFFFFF00);
const KERNEL_DATA: MemoryType = MemoryType::custom(0xFFFFFF01);

#[cfg(debug_assertions)]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Debug);
}

#[cfg(not(debug_assertions))]
fn configure_log_level() {
    log::set_max_level(log::LevelFilter::Info);
}

pub fn get_protocol<P: Protocol>(boot_services: &BootServices, handle: Handle) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.handle_protocol(handle))
}

pub fn locate_protocol<P: Protocol>(boot_services: &BootServices) -> Option<&mut P> {
    acquire_protocol_unwrapped(boot_services.locate_protocol::<P>())
}

fn acquire_protocol_unwrapped<P: Protocol>(result: uefi::Result<&UnsafeCell<P>>) -> Option<&mut P> {
    if let Ok(completion) = result {
        if completion.status() == Status::SUCCESS {
            Some(unsafe { &mut *(completion.unwrap().get()) })
        } else {
            None
        }
    } else {
        None
    }
}

pub struct PointerBuffer<'buf> {
    pub pointer: *mut u8,
    pub buffer: &'buf mut [u8],
}

#[allow(dead_code)]
pub fn allocate_pool(
    boot_services: &BootServices,
    memory_type: MemoryType,
    size: usize,
) -> PointerBuffer {
    let alloc_pointer = match boot_services.allocate_pool(memory_type, size) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    };
    let alloc_buffer = unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, size) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

#[allow(dead_code)]
pub fn allocate_pages(
    boot_services: &BootServices,
    allocate_type: AllocateType,
    memory_type: MemoryType,
    pages_count: usize,
) -> PointerBuffer {
    if let AllocateType::MaxAddress(address) = allocate_type {
        if (address % PAGE_SIZE) != 0x0 {
            panic!("Address is not page-aligned ({})", address)
        }
    }

    let alloc_pointer = match boot_services.allocate_pages(allocate_type, memory_type, pages_count)
    {
        Ok(completion) => completion.unwrap() as *mut u8,
        Err(error) => panic!("{:?}", error),
    };

    let alloc_buffer =
        unsafe { &mut *slice_from_raw_parts_mut(alloc_pointer, pages_count * PAGE_SIZE) };

    PointerBuffer {
        pointer: alloc_pointer,
        buffer: alloc_buffer,
    }
}

#[allow(dead_code)]
pub fn free_pool(boot_services: &BootServices, buffer: PointerBuffer) {
    match boot_services.free_pool(buffer.pointer) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

#[allow(dead_code)]
pub fn free_pages(boot_services: &BootServices, buffer: PointerBuffer, count: usize) {
    match boot_services.free_pages(buffer.pointer as u64, count) {
        Ok(completion) => completion.unwrap(),
        Err(error) => panic!("{:?}", error),
    }
}

/// returns the minimum necessary memory pages to contain the given size in bytes.
pub fn aligned_slices(size: usize, alignment: usize) -> usize {
    ((size + alignment) - 1) / alignment
}

pub fn open_file<F: File>(file: &mut F, name: &str) -> RegularFile {
    debug!("Attempting to load file system object: {}", name);
    match file.open(name, FileMode::Read, FileAttribute::READ_ONLY) {
        // this is unsafe due to the possibility of passing an invalid file handle to external code
        Ok(completion) => unsafe { RegularFile::new(completion.expect("failed to find file")) },
        Err(error) => panic!("{:?}", error),
    }
}

pub fn read_file(file: &mut RegularFile, position: u64, buffer: &mut [u8]) {
    debug!("Reading file contents into memory (pos {}).", position);
    file.set_position(position)
        .expect_success("failed to set position of file");
    file.read(buffer)
        .expect_success("failed to read file into memory");
}

#[entry]
fn efi_main(image_handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("failed to unwrap UEFI services");
    info!("Loaded Gsai UEFI bootloader v{}.", VERSION);

    configure_log_level();
    info!("Configured log level to '{:?}'.", log::max_level());
    info!("Configuring bootloader environment.");

    // this ugly little hack is to sever the boot_services' lifetime from the system_table, allowing us
    // to later move the system_table into `kernel_transfer()`
    let boot_services = unsafe { &*(system_table.boot_services() as *const BootServices) };
    info!("Acquired boot services from UEFI firmware.");

    // test to see how much memory we're working with
    ensure_enough_memory(boot_services);

    // acquire graphics output to ensure a gout device
    let framebuffer = match locate_protocol::<GraphicsOutput>(boot_services) {
        Some(graphics_output) => {
            let pointer = graphics_output.frame_buffer().as_mut_ptr() as *mut u8;
            let mode = select_graphics_mode(graphics_output);
            let mode_info = mode.info();
            info!("Selected graphics mode: {:?}", mode_info);
            let resolution = mode_info.resolution();
            let size = efi_boot::Size {
                width: resolution.0,
                height: resolution.1,
            };
            info!("Acquired and configured graphics output protocol.");

            Some(FramebufferPointer { pointer, size })
        }
        None => {
            warn!("No graphics output found. Kernel will default to using serial output.");
            None
        }
    };

    // prepare required environment data
    let image = get_protocol::<LoadedImage>(boot_services, image_handle)
        .expect("failed to acquire boot image");
    info!("Acquired boot image from boot services.");
    let device_path = get_protocol::<DevicePath>(boot_services, image.device())
        .expect("failed to acquire boot image device path");
    info!("Acquired boot image device path.");
    let file_handle = boot_services
        .locate_device_path::<SimpleFileSystem>(device_path)
        .expect_success("failed to acquire file handle from device path");
    info!("Acquired file handle from device path.");
    let file_system = get_protocol::<SimpleFileSystem>(boot_services, file_handle)
        .expect("failed to load file system from file handle");
    info!("Acquired file system protocol from file handle.");
    let root_directory = &mut file_system
        .open_volume()
        .expect_success("failed to open boot file system root directory");
    info!("Loaded boot file system root directory.");

    // load kernel
    let kernel_file = acquire_kernel_file(root_directory);
    info!("Acquired kernel image file.");
    let kernel_entry_point = load_kernel(boot_services, kernel_file);

    kernel_transfer(image_handle, system_table, kernel_entry_point, framebuffer)
}

fn ensure_enough_memory(boot_services: &BootServices) {
    let mmap_size_bytes = boot_services.memory_map_size() + (size_of::<MemoryDescriptor>() * 2);
    let mmap_buffer = allocate_pool(boot_services, MemoryType::LOADER_DATA, mmap_size_bytes);
    let total_memory: usize = match boot_services.memory_map(mmap_buffer.buffer) {
        Ok(completion) => completion.unwrap().1,
        Err(error) => panic!("{:?}", error),
    }
    .map(|descriptor| (descriptor.page_count as usize) * PAGE_SIZE)
    .sum::<usize>();

    if total_memory < MINIMUM_MEMORY {
        panic!(
            "system does not have the minimum required {}MB of RAM.",
            MINIMUM_MEMORY / (1024 * 1024)
        );
    } else {
        info!(
            "Verified minimum system memory: {}MB",
            MINIMUM_MEMORY / (1024 * 1024)
        );
    }

    free_pool(boot_services, mmap_buffer);
}

fn select_graphics_mode(graphics_output: &mut GraphicsOutput) -> Mode {
    let graphics_mode = graphics_output
        .modes()
        .map(|mode| mode.expect("warning encountered while querying mode"))
        .last()
        .unwrap();

    graphics_output
        .set_mode(&graphics_mode)
        .expect_success("failed to set graphics mode");

    graphics_mode
}

fn acquire_kernel_file(root_directory: &mut Directory) -> RegularFile {
    let mut kernel_directory = open_file(root_directory, "EFI");
    let mut gsai_directory = open_file(&mut kernel_directory, "gsai");
    let kernel_file = open_file(&mut gsai_directory, "kernel.elf");
    kernel_directory.close();
    gsai_directory.close();
    kernel_file
}

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
                KERNEL_CODE,
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

fn kernel_transfer(
    image_handle: Handle,
    system_table: SystemTable<Boot>,
    kernel_entry_point: usize,
    framebuffer: Option<FramebufferPointer>,
) -> ! {
    info!("Preparing to exit boot services environment.");
    // Retrieve a raw allocation pointer & size for the system memory map.
    //
    // Remark:
    //  We can't use `memory::allocate_pool`, because the `system_table.boot_services()` would have its lifetime
    //  used to provide a lifetime to the returned pointer buffer. This is a problem because `system_table` has
    //  to be moved into `ExitBootServices`, which isn't possible if `boot_services()` has had its lifetime used
    //  elsewhere.
    let (mmap_ptr, mmap_alloc_size) = {
        let boot_services = system_table.boot_services();
        // Determine the total allocation size of the memory map, in bytes (+ to cover any extraneous entries created before `ExitBootServices`).
        let mmap_alloc_size = boot_services.memory_map_size() + (4 * size_of::<MemoryDescriptor>());
        let alloc_ptr = match boot_services.allocate_pool(KERNEL_DATA, mmap_alloc_size) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

        (alloc_ptr, mmap_alloc_size)
    };

    info!("Finalizing exit from boot services environment.");
    system_table
        .stdout()
        .reset(false)
        .expect_success("failed to reset standard output");

    // Create the byte buffer to the used for filling in memory descriptors. This buffer, on the call to `ExitBootServices`, provides
    // lifetime information, and so cannot be reinterpreted easily.
    let mmap_buffer = unsafe { &mut *slice_from_raw_parts_mut(mmap_ptr, mmap_alloc_size) };
    // After this point point, the previous system_table and boot_services are no longer valid
    let (runtime_table, mmap_iter) =
        match system_table.exit_boot_services(image_handle, mmap_buffer) {
            Ok(completion) => completion.unwrap(),
            Err(error) => panic!("{:?}", error),
        };

    // Remark: For some reason, this cast itself doesn't result in a valid memory map, even provided
    //  the alignment is correct—so we have to read in the memory descriptors.
    //
    // This could be due to the actual entry size not being equal to size_of::<MemoryDescriptor>().
    let memory_map = unsafe {
        &mut *slice_from_raw_parts_mut(mmap_ptr as *mut MemoryDescriptor, mmap_iter.len())
    };
    for (index, descriptor) in mmap_iter.enumerate() {
        memory_map[index] = *descriptor;
    }

    // Finally, drop into the kernel.
    let kernel_main: efi_boot::KernelMain = unsafe { transmute(kernel_entry_point) };
    let boot_info = efi_boot::BootInfo::new(unsafe { memory_map.align_to().1 }, framebuffer);
    let return_code = kernel_main(boot_info);
    let return_status: Status = unsafe { transmute(return_code) };

    unsafe { runtime_table.runtime_services() }.reset(ResetType::Shutdown, return_status, None)
}
