mod slob;

pub use slob::*;

use libkernel::memory::{FrameManager, PageManager};
use libkernel::{Address, Virtual};
use spin::Once;

static LIMINE_MMAP: limine::LimineMmapRequest = limine::LimineMmapRequest::new(crate::LIMINE_REV);
static LIMINE_HHDM: limine::LimineHhdmRequest = limine::LimineHhdmRequest::new(crate::LIMINE_REV);

fn get_limine_mmap() -> &'static [limine::LimineMemmapEntry] {
    LIMINE_MMAP
        .get_response()
        .get()
        .expect("bootloader provided no memory map response")
        .mmap()
        .expect("bootloader provided no memory map entries")
}

static HHDM_ADDR: Once<Address<Virtual>> = Once::new();
// Gets the kernel's higher half direct mapping page.
pub fn get_kernel_hhdm_addr() -> Address<Virtual> {
    *HHDM_ADDR.call_once(|| {
        Address::<Virtual>::new(
            LIMINE_HHDM.get_response().get().expect("bootloader provided no higher-half direct mapping").offset
                as usize,
        )
        .expect("bootloader provided an invalid higher-half direct mapping address")
    })
}

static KERNEL_FRAME_MANAGER: Once<FrameManager> = Once::new();
/// Gets the kernel frame manager.
pub fn get_kernel_frame_manager() -> &'static FrameManager<'static> {
    KERNEL_FRAME_MANAGER.call_once(|| FrameManager::from_mmap(get_limine_mmap(), get_kernel_hhdm_addr()))
}

use libkernel::LinkerSymbol;

extern "C" {
    static __text_start: LinkerSymbol;
    static __text_end: LinkerSymbol;

    static __rodata_start: LinkerSymbol;
    static __rodata_end: LinkerSymbol;

    static __bss_start: LinkerSymbol;
    static __bss_end: LinkerSymbol;

    static __data_start: LinkerSymbol;
    static __data_end: LinkerSymbol;
}

static KERNEL_PAGE_MANAGER: Once<PageManager> = Once::new();
/// Gets the kernel page manager.
pub fn init_kernel_page_manager() {
    KERNEL_PAGE_MANAGER.call_once(|| unsafe {
        use libkernel::memory::*;

        let hhdm_base_page_index = get_kernel_hhdm_addr().page_index();
        let frame_manager = get_kernel_frame_manager();
        let hhdm_mapped_page = Page::from_index(hhdm_base_page_index);
        let old_page_manager = PageManager::from_current(&hhdm_mapped_page);
        let page_manager = PageManager::new(frame_manager, &hhdm_mapped_page, None);

        // map code
        (__text_start.as_usize()..__text_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                trace!("TEXT     {:?}", page);

                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RX | PageAttributes::GLOBAL,
                        frame_manager,
                    )
                    .unwrap()
            });

        // map readonly
        (__rodata_start.as_usize()..__rodata_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                trace!("RODATA   {:?}", page);

                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RO | PageAttributes::GLOBAL,
                        frame_manager,
                    )
                    .unwrap()
            });

        // map readwrite
        (__bss_start.as_usize()..__bss_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                trace!("BSS      {:?}", page);

                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RW | PageAttributes::GLOBAL,
                        frame_manager,
                    )
                    .unwrap()
            });

        (__data_start.as_usize()..__data_end.as_usize())
            .step_by(0x1000)
            .map(|page_base_addr| Page::from_index(page_base_addr / 0x1000))
            .for_each(|page| {
                trace!("DATA     {:?}", page);

                page_manager
                    .map(
                        &page,
                        old_page_manager.get_mapped_to(&page).unwrap(),
                        false,
                        PageAttributes::RW | PageAttributes::GLOBAL,
                        frame_manager,
                    )
                    .unwrap()
            });

        for entry in get_limine_mmap() {
            let entry_start = entry.base as usize;
            let entry_end = entry_start + (entry.len as usize);
            let page_attributes = {
                use libkernel::memory::PageAttributes;
                use limine::LimineMemoryMapEntryType;
                match entry.typ {
                    LimineMemoryMapEntryType::BadMemory => PageAttributes::empty(),

                    LimineMemoryMapEntryType::KernelAndModules
                    | LimineMemoryMapEntryType::Usable
                    | LimineMemoryMapEntryType::Reserved
                    | LimineMemoryMapEntryType::BootloaderReclaimable
                    | LimineMemoryMapEntryType::AcpiReclaimable => PageAttributes::RW,

                    LimineMemoryMapEntryType::AcpiNvs | LimineMemoryMapEntryType::Framebuffer => PageAttributes::MMIO,
                }
            };

            for page_base_addr in (entry_start..entry_end).step_by(0x1000) {
                let frame_index = page_base_addr / 0x1000;
                let page_index = hhdm_base_page_index + frame_index;

                page_manager
                    .map(&Page::from_index(page_index), frame_index, false, page_attributes, frame_manager)
                    .unwrap();
            }
        }

        debug!("Switching to kernel page tables...");
        page_manager.write_cr3();

        page_manager
    });
}

pub fn get_kernel_page_manager() -> &'static PageManager {
    KERNEL_PAGE_MANAGER.get().unwrap()
}

pub fn reclaim_bootloader_memory() {
    let frame_manager = get_kernel_frame_manager();

    get_limine_mmap()
        .iter()
        .filter(|entry| entry.typ == limine::LimineMemoryMapEntryType::BootloaderReclaimable)
        .flat_map(|entry| (entry.base..(entry.base + entry.len)))
        .step_by(0x1000)
        .map(|base_frame_addr| (base_frame_addr / 0x1000) as usize)
        .for_each(|frame_index| {
            frame_manager.force_modify_type(frame_index, libkernel::memory::FrameType::Usable).unwrap()
        })
}
