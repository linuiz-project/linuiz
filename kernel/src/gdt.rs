use lib::cell::SyncOnceCell;
use x86_64::{registers::segmentation::SegmentSelector, structures::gdt::GlobalDescriptorTable};

extern "C" {
    static __gdt: lib::LinkerSymbol;
}

lazy_static::lazy_static! {
    static ref GDT: GlobalDescriptorTable = unsafe {
        use x86_64::structures::gdt::Descriptor;

        let mut gdt = GlobalDescriptorTable::from_raw_slice(core::slice::from_raw_parts(__gdt.as_ptr(), 1 /* Null Seg */));

        // This GDT layout is very specific, due to the behaviour of the IA32_STAR MSR and its
        // affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
        // standard set by the aforementioned IA32_STAR MSR.
        //
        // Details can be found in the description of the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
        KCODE_SELECTOR.set(gdt.add_entry(Descriptor::kernel_code_segment())).unwrap();
        KDATA_SELECTOR.set(gdt.add_entry(Descriptor::kernel_data_segment())).unwrap();
        UDATA_SELECTOR.set(gdt.add_entry(Descriptor::user_data_segment())).unwrap();
        UCODE_SELECTOR.set(gdt.add_entry(Descriptor::user_code_segment())).unwrap();
        TSS_SELECTOR.set(gdt.add_entry(Descriptor::tss_segment(
            &lib::structures::gdt::TSS,
        ))).unwrap();

        gdt
    };
}

pub static mut KCODE_SELECTOR: SyncOnceCell<SegmentSelector> = SyncOnceCell::new();
pub static mut KDATA_SELECTOR: SyncOnceCell<SegmentSelector> = SyncOnceCell::new();
pub static mut UCODE_SELECTOR: SyncOnceCell<SegmentSelector> = SyncOnceCell::new();
pub static mut UDATA_SELECTOR: SyncOnceCell<SegmentSelector> = SyncOnceCell::new();
pub static mut TSS_SELECTOR: SyncOnceCell<SegmentSelector> = SyncOnceCell::new();

pub fn init() {
    unsafe {
        GDT.load();

        use x86_64::instructions::{
            segmentation::{Segment, CS, DS, ES, FS, GS, SS},
            tables::load_tss,
        };
        CS::set_reg(*KCODE_SELECTOR.get().unwrap());
        SS::set_reg(*KDATA_SELECTOR.get().unwrap());

        // Because this is x86, everything is complicated. It's important we load the extra
        // data segment registers with the null descriptor, because if they don't point to a
        // null descriptor, then when CPL changes, the processor will clear the base and limit
        // of the relevant descriptor.
        //
        // This has the fun behavioural side-effect of *also* clearing the IA32_FS/GS_BASE MSRs,
        // thus making any code involved in the CPL change context unable to access thread-local or
        // process-local state.
        let null_selector = SegmentSelector::new(0x0, x86_64::PrivilegeLevel::Ring0);
        ES::set_reg(null_selector);
        DS::set_reg(null_selector);
        FS::set_reg(null_selector);
        GS::set_reg(null_selector);
        load_tss(*TSS_SELECTOR.get().unwrap());
    }
}
