use spin::Once;

pub use x86_64::{
    instructions::tables::{lgdt, sgdt},
    registers::segmentation::SegmentSelector,
    structures::gdt::*,
};

static GDT: spin::Lazy<GlobalDescriptorTable> = spin::Lazy::new(|| {
    let mut gdt = GlobalDescriptorTable::new();

    // This GDT layout is very specific, due to the behaviour of the IA32_STAR MSR and its
    // affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
    // standard set by the aforementioned IA32_STAR MSR.
    //
    // Details can be found in the description of the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
    KCODE_SELECTOR.call_once(|| gdt.add_entry(Descriptor::kernel_code_segment()));
    KDATA_SELECTOR.call_once(|| gdt.add_entry(Descriptor::kernel_data_segment()));
    UDATA_SELECTOR.call_once(|| gdt.add_entry(Descriptor::user_data_segment()));
    UCODE_SELECTOR.call_once(|| gdt.add_entry(Descriptor::user_code_segment()));

    gdt
});

pub static KCODE_SELECTOR: Once<SegmentSelector> = Once::new();
pub static KDATA_SELECTOR: Once<SegmentSelector> = Once::new();
pub static UCODE_SELECTOR: Once<SegmentSelector> = Once::new();
pub static UDATA_SELECTOR: Once<SegmentSelector> = Once::new();

pub fn load() {
    // SAFETY:  This would technically be unsafe, but since we know the GDT's structure
    //          deterministically, running this function over and over would not change
    //          software execution at all. So, it's safe to execute all of this code.
    unsafe {
        GDT.load();

        use x86_64::instructions::segmentation::{Segment, CS, DS, ES, FS, GS, SS};

        CS::set_reg(*KCODE_SELECTOR.get().unwrap());
        SS::set_reg(*KDATA_SELECTOR.get().unwrap());

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
