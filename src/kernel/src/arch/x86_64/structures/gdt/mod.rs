///! Most of this is directly copied from the `x86_64` crate, found here: https://github.com/rust-osdev/x86_64
///! There are some slight modifications, to better suit the use-case of this OS.
mod segmentation;
pub use segmentation::*;

use core::arch::asm;

/// Set by the processor if this segment has been accessed. Only cleared by software.
/// _Setting_ this bit in software prevents GDT writes on first use.
const ACCESSED: u64 = 1 << 40;
/// For 32-bit data segments, sets the segment as writable. For 32-bit code segments,
/// sets the segment as _readable_. In 64-bit mode, ignored for all segments.
const WRITABLE: u64 = 1 << 41;
/// For code segments, sets the segment as “conforming”, influencing the
/// privilege checks that occur on control transfers. For 32-bit data segments,
/// sets the segment as "expand down". In 64-bit mode, ignored for data segments.
const CONFORMING: u64 = 1 << 42;
/// This flag must be set for code segments and unset for data segments.
const EXECUTABLE: u64 = 1 << 43;
/// This flag must be set for user segments (in contrast to system segments).
const USER_SEGMENT: u64 = 1 << 44;
/// These two bits encode the Descriptor Privilege Level (DPL) for this descriptor.
/// If both bits are set, the DPL is Ring 3, if both are unset, the DPL is Ring 0.
const DPL_RING_3: u64 = 3 << 45;
/// Must be set for any segment, causes a segment not present exception if not set.
const PRESENT: u64 = 1 << 47;
/// Available for use by the Operating System
const AVAILABLE: u64 = 1 << 52;
/// Must be set for 64-bit code segments, unset otherwise.
const LONG_MODE: u64 = 1 << 53;
/// Use 32-bit (as opposed to 16-bit) operands. If [`LONG_MODE`][Self::LONG_MODE] is set,
/// this must be unset. In 64-bit mode, ignored for data segments.
const DEFAULT_SIZE: u64 = 1 << 54;
/// Limit field is scaled by 4096 bytes. In 64-bit mode, ignored for all segments.
const GRANULARITY: u64 = 1 << 55;
/// Bits `0..=15` of the limit field (ignored in 64-bit mode)
const LIMIT_0_15: u64 = 0xFFFF;
/// Bits `16..=19` of the limit field (ignored in 64-bit mode)
const LIMIT_16_19: u64 = 0xF << 48;
/// Bits `0..=23` of the base field (ignored in 64-bit mode, except for fs and gs)
const BASE_0_23: u64 = 0xFF_FFFF << 16;
/// Bits `24..=31` of the base field (ignored in 64-bit mode, except for fs and gs)
const BASE_24_31: u64 = 0xFF << 56;

const COMMON_FLAGS: u64 = USER_SEGMENT | PRESENT | WRITABLE | ACCESSED | LIMIT_0_15 | LIMIT_16_19 | GRANULARITY;
const KERNEL_DATA_FLAGS: u64 = COMMON_FLAGS | DEFAULT_SIZE;
const KERNEL_CODE64_FLAGS: u64 = COMMON_FLAGS | EXECUTABLE | LONG_MODE;
const USER_DATA_FLAGS: u64 = COMMON_FLAGS | DEFAULT_SIZE | DPL_RING_3;
const USER_CODE64_FLAGS: u64 = COMMON_FLAGS | EXECUTABLE | LONG_MODE | DPL_RING_3;

/// The GDT layout is very specific, due to the behaviour of the **IA32_STAR** MSR and its
/// affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
/// standard set by the aforementioned **IA32_STAR** MSR. Details can be found in the description of
/// the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
///
/// Additionally, x86 requires that the first GDT entry be null (i.e. no segment information).
static GDT: [u64; 5] = [0, KERNEL_CODE64_FLAGS, KERNEL_DATA_FLAGS, USER_DATA_FLAGS, USER_CODE64_FLAGS];

pub const KCODE_SELECTOR: SegmentSelector = SegmentSelector::new(1, PrivilegeLevel::Ring0);
pub const KDATA_SELECTOR: SegmentSelector = SegmentSelector::new(2, PrivilegeLevel::Ring0);
pub const UDATA_SELECTOR: SegmentSelector = SegmentSelector::new(3, PrivilegeLevel::Ring3);
pub const UCODE_SELECTOR: SegmentSelector = SegmentSelector::new(4, PrivilegeLevel::Ring3);

/// ### Safety
///
/// This function should be executed only once, and prior to any point when the FS/GS _BASE MSRs will
/// be in use, as they are cleared when this function is run.
pub unsafe fn load() {
    let gdt_size = core::mem::size_of_val(&GDT);
    let gdt_ptr = crate::arch::x86_64::structures::DescriptorTablePointer {
        limit: u16::try_from(gdt_size).unwrap() - 1,
        base: GDT.as_ptr().addr().try_into().unwrap(),
    };

    // Safety: The GDT is properly formed, and the descriptor table pointer is
    //         set to the GDT's memory location, with the requisite limit set
    //         correctly (size in bytes, less 1).
    unsafe {
        asm!(
            "lgdt [{}]", in(reg) &gdt_ptr, options(readonly, nostack, preserves_flags)
        );
    }

    // Safety: This is special since we cannot directly move to CS; x86 requires the instruction
    //         pointer and CS to be set at the same time. To do this, we push the new segment selector
    //         and return value onto the stack and use a "far return" (`retfq`) to reload CS and
    //         continue at the end of our function.
    //
    //         Note we cannot use a "far call" (`lcall`) or "far jmp" (`ljmp`) to do this because then we
    //         would only be able to jump to 32-bit instruction pointers. Only Intel implements support
    //         for 64-bit far calls/jumps in long-mode, AMD does not.
    unsafe {
        asm!(
            "push {sel}",
            "lea {tmp}, [55f + rip]",
            "push {tmp}",
            "retfq",
            "55:",
            sel = in(reg) u64::from(KCODE_SELECTOR.as_u16()),
            tmp = lateout(reg) _,
            options(preserves_flags),
        );
    }

    // Safety: While setting the ES & DS segment registers to null is perfectly safe, setting
    //         the FS & GS segment registers (on Intel only, not AMD) clears the respective
    //         FS/GS base. Thus, it is imperative that this function not be run after the GS
    //         base has been loaded with the CPU thread-local state structure pointer.
    unsafe {
        // Because this is x86, everything is complicated. It's important we load the extra
        // data segment registers (FS/GS) with the null descriptors, because if they don't
        // point to a null descriptor, then when CPL changes, the processor will clear the
        // base and limit of the relevant descriptor.
        //
        // This has the fun behavioural side-effect of ALSO clearing the **IA32_FS/GS_BASE** MSRs,
        // thus making any code involved in the CPL change context unable to access thread-local or
        // process-local state (when those MSRs are in use for the purpose).
        asm!(
            "mov ss, {sel:x}",
            "mov es, 0",
            "mov ds, 0",
            "mov fs, 0",
            "mov gs, 0",
            sel = in(reg) KDATA_SELECTOR.as_u16(),
            options(nostack, preserves_flags)
        );
    }
}
