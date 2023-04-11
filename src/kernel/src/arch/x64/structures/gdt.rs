pub use x86_64::{
    instructions::tables::{lgdt, sgdt},
    registers::segmentation::SegmentSelector,
    structures::gdt::*,
};

struct GdtData {
    gdt: GlobalDescriptorTable,
    kcode: SegmentSelector,
    kdata: SegmentSelector,
    ucode: SegmentSelector,
    udata: SegmentSelector,
}

static GDT_DATA: spin::Lazy<GdtData> = spin::Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();

    // This GDT layout is very specific, due to the behaviour of the IA32_STAR MSR and its
    // affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
    // standard set by the aforementioned IA32_STAR MSR.
    //
    // Details can be found in the description of the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
    let kcode = gdt.add_entry(Descriptor::kernel_code_segment());
    let kdata = gdt.add_entry(Descriptor::kernel_data_segment());
    let ucode = gdt.add_entry(Descriptor::user_data_segment());
    let udata = gdt.add_entry(Descriptor::user_code_segment());

    GdtData { gdt, kcode, kdata, ucode, udata }
});

#[inline]
pub fn kernel_code_selector() -> SegmentSelector {
    GDT_DATA.kcode
}

#[inline]
pub fn kernel_data_selector() -> SegmentSelector {
    GDT_DATA.kdata
}

#[inline]
pub fn user_code_selector() -> SegmentSelector {
    GDT_DATA.ucode
}

#[inline]
pub fn user_data_selector() -> SegmentSelector {
    GDT_DATA.udata
}

pub fn load() {
    // Safety:  This would technically be unsafe, but since we know the GDT's structure
    //          deterministically, running this function over and over would not change
    //          software execution at all. So, it's safe to execute all of this code.
    unsafe {
        GDT_DATA.gdt.load();

        use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};

        CS::set_reg(kernel_code_selector());
        SS::set_reg(kernel_data_selector());

        // Because this is x86, everything is complicated. It's important we load the extra
        // data segment registers (fs/gs) with the null descriptors, because if they don't
        // point to a null descriptor, then when CPL changes, the processor will clear the
        // base and limit of the relevant descriptor.
        //
        // This has the fun behavioural side-effect of *also* clearing the IA32_FS/GS_BASE MSRs,
        // thus making any code involved in the CPL change context unable to access thread-local or
        // process-local state.
        let null_selector = SegmentSelector::new(0x0, x86_64::PrivilegeLevel::Ring0);
        ES::set_reg(null_selector);
        DS::set_reg(null_selector);
        // It should be noted that Intel (not AMD) clears the FS/GS base when loading a null selector.
        FS::set_reg(null_selector);
        GS::set_reg(null_selector);
    }
}
