use crate::arch::x86_64::structures::{DescriptorTablePointer, tss::TaskStateSegment};
use bit_field::BitField;
use core::ops::Range;
use spin::Once;

pub static KCODE_SELECTOR: Once<SegmentSelector> = Once::new();
pub static KDATA_SELECTOR: Once<SegmentSelector> = Once::new();
pub static UDATA_SELECTOR: Once<SegmentSelector> = Once::new();
pub static UCODE_SELECTOR: Once<SegmentSelector> = Once::new();

crate::singleton! {
    #[derive(Debug, Clone)]
    #[repr(C, align(8))]
    pub GlobalDescriptorTable {
        table: [u64; 7],
        len: usize,
    }

    fn init() {
        let mut gdt = Self::empty();

        // The GDT layout is very specific, due to the behaviour of the `IA32_STAR` MSR and its
        // affect on syscalls. Do not change this, or if it is changed, ensure it follows the requisite
        // standard set by the aforementioned `IA32_STAR` MSR. Details can be found in the description of
        // the `syscall` and `sysret` instructions in the IA32 Software Developer's Manual.
        let kcode_selector = gdt.append_segment(GenericSegmentDescriptor::kernel_code());
        let kdata_selector = gdt.append_segment(GenericSegmentDescriptor::kernel_data());
        let udata_selector = gdt.append_segment(GenericSegmentDescriptor::user_data());
        let ucode_selector = gdt.append_segment(GenericSegmentDescriptor::user_code());

        KCODE_SELECTOR.call_once(|| kcode_selector);
        KDATA_SELECTOR.call_once(|| kdata_selector);
        UDATA_SELECTOR.call_once(|| udata_selector);
        UCODE_SELECTOR.call_once(|| ucode_selector);

        trace!("Segment descriptors loaded:");
        trace!("Kernel code: {kcode_selector:?}");
        trace!("Kernel data: {kdata_selector:?}");
        trace!("User data: {udata_selector:?}");
        trace!("User code: {ucode_selector:?}");

        gdt
    }
}

impl GlobalDescriptorTable {
    /// An empty [`GlobalDescriptorTable`].
    fn empty() -> Self {
        Self {
            table: [0; _],

            // x86 requires that the first GDT entry remain null.
            len: 1,
        }
    }

    pub fn load_static() {
        let static_gdt = Self::get_static();

        // Safety: The GDT is properly formed, and the descriptor table pointer is
        //         set to the GDT's memory location, with the requisite limit set
        //         correctly (size in bytes, less 1).
        unsafe {
            static_gdt.load();
        }

        let kcode_selector = *KCODE_SELECTOR.wait();
        let kdata_selector = *KDATA_SELECTOR.wait();

        trace!("Jumping to the new code segment: {kcode_selector:?}");
        // Safety: This is special since we cannot directly move to CS; x86 requires the instruction
        //         pointer and CS to be set at the same time. To do this, we push the new segment selector
        //         and return value onto the stack and use a "far return" (`retfq`) to reload CS and
        //         continue at the end of our function.
        //
        //         Note we cannot use a "far call" (`lcall`) or "far jmp" (`ljmp`) to do this because then we
        //         would only be able to jump to 32-bit instruction pointers. Only Intel implements support
        //         for 64-bit far calls/jumps in long-mode, AMD does not.
        unsafe {
            core::arch::asm!(
                "
                push {selector}
                lea {ip}, [55f + rip]
                push {ip}
                retfq
                55:
                ",
                selector = in(reg) u64::from(kcode_selector.as_u16()),
                ip = out(reg) _,
                options(preserves_flags),
            );
        }

        trace!("Clearing extant segment registers...");
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
            // This has the fun behavioural side-effect of ALSO clearing the FS/GS _BASE MSRs,
            // thus making any code involved in the CPL change context unable to access thread-local or
            // process-local state (when those MSRs are in use for the purpose).
            core::arch::asm!(
                "
                mov ss, {selector:x}

                push rax        # store `rax`
                xor rax, rax    # zero-out `rax`

                # zero-out segment registers
                mov es, ax
                mov ds, ax
                mov fs, ax
                mov gs, ax

                pop rax         # restore `rax`
                ",
                selector = in(reg) kdata_selector.as_u16(),
                options(preserves_flags)
            );
        }

        trace!("Finished loading static global descriptor table.");
    }

    /// # Safety
    ///
    /// - An invalid [`GlobalDescriptorTable`] could potentially make memory unreadable or unwriteable.
    /// - This should be executed prior to any point when the FS/GS _BASE MSRs will
    ///   be in use, as they are cleared when this function is run.
    unsafe fn load(&self) {
        use core::arch::asm;

        let dtptr = DescriptorTablePointer::from(self);

        trace!(
            "Loading: {:X?}:\n{self:#X?}\n{dtptr:#X?}",
            core::ptr::from_ref(self)
        );

        // Safety: The GDT is properly formed, and the descriptor table pointer is
        //         set to the GDT's memory location, with the requisite limit set
        //         correctly (size in bytes, less 1).
        unsafe {
            asm!(
                "lgdt [{}]",
                in(reg) &raw const dtptr,
                options(readonly, nostack, preserves_flags)
            );
        }
    }

    /// Appends a [`SegmentDescriptor`] to the [`GlobalDescriptorTable`] entries table.
    pub fn append_segment(
        &mut self,
        segment_descriptor: impl SegmentDescriptor,
    ) -> SegmentSelector {
        let current_index = self.len;
        let privilege_level = segment_descriptor.privilege_level();
        let appended_entry_count = segment_descriptor.append_entries(&mut self.table[self.len..]);
        self.len += appended_entry_count;

        SegmentSelector::new(u16::try_from(current_index).unwrap(), privilege_level)
    }

    pub fn with_temporary<T>(func: impl FnOnce(&mut Self) -> T) -> T {
        let static_gdt = Self::get_static();

        let mut temp_gdt = static_gdt.clone();

        crate::interrupts::uninterruptable(|| {
            // Load the temporary GDT for loading TSS.
            // Safety: Temporary GDT is identical to static GDT + 1 entry, so cannot
            //         cause undefined behaviour by loading.
            unsafe {
                temp_gdt.load();
            }

            let value = func(&mut temp_gdt);

            // Safety: Loading the static GDT is always safe.
            unsafe {
                static_gdt.load();
            }

            value
        })
    }
}

/// Specifies which element to load into a segment from
/// descriptor tables (i.e., is a index to LDT or GDT table
/// with some additional flags).
///
/// See Intel 3a, Section 3.4.2 "Segment Selectors"
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SegmentSelector(u16);

impl SegmentSelector {
    /// Can be used as a selector into a non-existent segment and assigned to segment registers,
    /// e.g. data segment register in ring 0
    pub const NULL: Self = Self(0);

    /// Creates a new [`SegmentSelector`]
    pub fn new(index: u16, rpl: PrivilegeLevel) -> SegmentSelector {
        SegmentSelector(index << 3 | u16::from(rpl))
    }

    /// Returns the selector as a raw u16.
    pub fn as_u16(self) -> u16 {
        self.0
    }

    /// Returns the GDT index.
    pub fn gdt_index(self) -> u16 {
        self.0 >> 3
    }

    /// Returns the requested privilege level.
    pub fn privilege_level(self) -> PrivilegeLevel {
        PrivilegeLevel::try_from(self.0 & 0b11).unwrap()
    }
}

impl core::fmt::Debug for SegmentSelector {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_tuple("SegmentSelector")
            .field(&self.gdt_index())
            .field(&self.privilege_level())
            .finish()
    }
}

/// Represents a protection ring level.
#[repr(u16)]
#[derive(Debug, TryFromPrimitive, IntoPrimitive, Copy, Clone, PartialEq, Eq, Hash)]
pub enum PrivilegeLevel {
    /// Privilege-level 0 (most privilege): This level is used by critical system-software
    /// components that require direct access to, and control over, all processor and system
    /// resources. This can include BIOS, memory-management functions, and interrupt handlers.
    Ring0 = 0b00,

    /// Privilege-level 1 (moderate privilege): This level is used by less-critical system-
    /// software services that can access and control a limited scope of processor and system
    /// resources. Software running at these privilege levels might include some device drivers
    /// and library routines. The actual privileges of this level are defined by the
    /// operating system.
    Ring1 = 0b01,

    /// Privilege-level 2 (moderate privilege): Like level 1, this level is used by
    /// less-critical system-software services that can access and control a limited scope of
    /// processor and system resources. The actual privileges of this level are defined by the
    /// operating system.
    Ring2 = 0b10,

    /// Privilege-level 3 (least privilege): This level is used by application software.
    /// Software running at privilege-level 3 is normally prevented from directly accessing
    /// most processor and system resources. Instead, applications request access to the
    /// protected processor and system resources by calling more-privileged service routines
    /// to perform the accesses.
    Ring3 = 0b11,
}

/// Common bits between all kinds of segment descriptors.
///
/// Composed of:
/// - **Limit**, bits 0..16 and 48..52
/// - **Present**, bit 47
/// - **Granularity**, bit 55
const COMMON_BITS: u64 = {
    (1 << 47)               // Present
    | (1 << 55)             // Granularity
    | 0xFFFF | (0xF << 48) // Limit
};

/// Desired privilege level for the segment.
const PRIVILEGE_LEVEL_BIT_RANGE: Range<usize> = 45..47;

pub trait SegmentDescriptor {
    /// The desired privilege level of the segment.
    fn privilege_level(&self) -> PrivilegeLevel;

    /// Appends this [`SegmentDescriptor`]'s entries to the provided `append_to`
    /// slice, returning how many entries were appended.
    fn append_entries(self, append_to: &mut [u64]) -> usize;
}

/// A generic (non-system) segment descriptor.
pub struct GenericSegmentDescriptor(u64);

impl GenericSegmentDescriptor {
    /// Set by the processor if this segment has been accessed. Only cleared by software.
    /// Setting this bit in software prevents GDT writes on first use.
    ///
    /// Usually, this bit is set in 64-bit descriptors. Only unset if otherwise required.
    const ACCESSED_BIT_INDEX: usize = 40;

    /// For data segments, sets the segment as **writable**.
    ///
    /// For code segments, sets the segment as **readable**.
    const READ_WRITE_BIT_INDEX: usize = 41;

    /// This flag must be set for all non-system segments.
    const NON_SYSTEM_SEGMENT_BIT_INDEX: usize = 44;

    /// This flag must be set for code segments and unset for data segments.
    const EXECUTABLE_BIT_INDEX: usize = 43;

    /// Must be set for 64-bit code segments, unset otherwise.
    const LONG_MODE_CODE_BIT_INDEX: usize = 53;

    /// Use 32-bit (as opposed to 16-bit) operands. If [`LONG_MODE_CODE_BIT`][Self::LONG_MODE_CODE_BIT] is set,
    /// this must be unset. In 64-bit mode, ignored for data segments.
    const EXTENDED_SIZE_BIT_INDEX: usize = 54;

    fn new(is_code: bool, privilege_level: PrivilegeLevel) -> Self {
        let mut value = COMMON_BITS;

        value.set_bit(Self::ACCESSED_BIT_INDEX, true);
        value.set_bit(Self::READ_WRITE_BIT_INDEX, true);
        value.set_bit(Self::NON_SYSTEM_SEGMENT_BIT_INDEX, true);

        if is_code {
            value.set_bit(Self::EXECUTABLE_BIT_INDEX, true);
            value.set_bit(Self::LONG_MODE_CODE_BIT_INDEX, true);
        } else {
            value.set_bit(Self::EXTENDED_SIZE_BIT_INDEX, true);
        }

        value.set_bits(
            PRIVILEGE_LEVEL_BIT_RANGE,
            u64::from(u16::from(privilege_level)),
        );

        Self(value)
    }

    pub fn kernel_code() -> Self {
        Self::new(true, PrivilegeLevel::Ring0)
    }

    pub fn kernel_data() -> Self {
        Self::new(false, PrivilegeLevel::Ring0)
    }

    pub fn user_code() -> Self {
        Self::new(true, PrivilegeLevel::Ring3)
    }

    pub fn user_data() -> Self {
        Self::new(false, PrivilegeLevel::Ring3)
    }
}

impl SegmentDescriptor for GenericSegmentDescriptor {
    fn privilege_level(&self) -> PrivilegeLevel {
        let raw_bits = self.0.get_bits(PRIVILEGE_LEVEL_BIT_RANGE);
        let raw_bits_u16 = u16::try_from(raw_bits).unwrap();

        PrivilegeLevel::try_from(raw_bits_u16).unwrap()
    }

    fn append_entries(self, append_to: &mut [u64]) -> usize {
        append_to[0] = self.0;

        1
    }
}

/// A system segment descriptor.
pub struct SystemSegmentDescriptor(u128);

impl SystemSegmentDescriptor {
    const BASE_BIT_RANGE_1: Range<usize> = 16..32;
    const BASE_BIT_RANGE_2: Range<usize> = 56..64;
    const BASE_BIT_RANGE_3: Range<usize> = 56..64;
    const BASE_BIT_RANGE_4: Range<usize> = 64..96;

    const SYSTEM_SEGMENT_TYPE_BIT_RANGE: Range<usize> = 40..44;
    const SYSTEM_SEGMENT_TYPE_TSS_AVAILABLE: u128 = 0x9;

    /// Constructs a [`SystemSegmentDescriptor`] from a valid [`TaskStateSegment`].
    pub fn from_tss(tss: &TaskStateSegment) -> Self {
        let tss_ptr = core::ptr::from_ref(tss);

        let mut value = u128::from(COMMON_BITS);

        // Set the systems segment type (in this case, 64-bit TSS + Available)
        value.set_bits(
            Self::SYSTEM_SEGMENT_TYPE_BIT_RANGE,
            Self::SYSTEM_SEGMENT_TYPE_TSS_AVAILABLE,
        );

        // Set the privilege level to Ring 0.
        value.set_bits(
            PRIVILEGE_LEVEL_BIT_RANGE,
            u128::from(u16::from(PrivilegeLevel::Ring0)),
        );

        // Set all of the bits for the "base" of the table. And because of legacy
        // compatibility, this requires setting 4 different bit ranges.
        let base = u128::try_from(tss_ptr.addr()).unwrap();
        value.set_bits(Self::BASE_BIT_RANGE_1, base.get_bits(0..16));
        value.set_bits(Self::BASE_BIT_RANGE_2, base.get_bits(16..24));
        value.set_bits(Self::BASE_BIT_RANGE_3, base.get_bits(24..32));
        value.set_bits(Self::BASE_BIT_RANGE_4, base.get_bits(32..64));

        Self(value)
    }
}

impl SegmentDescriptor for SystemSegmentDescriptor {
    fn privilege_level(&self) -> PrivilegeLevel {
        let raw_bits = self.0.get_bits(PRIVILEGE_LEVEL_BIT_RANGE);
        let raw_bits_u16 = u16::try_from(raw_bits).unwrap();

        PrivilegeLevel::try_from(raw_bits_u16).unwrap()
    }

    #[allow(clippy::as_conversions, clippy::cast_possible_truncation)]
    fn append_entries(self, append_to: &mut [u64]) -> usize {
        append_to[0] = self.0 as u64;
        append_to[1] = (self.0 >> u64::BITS) as u64;

        2
    }
}
