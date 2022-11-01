#[cfg(target_arch = "x86_64")]
pub fn setup() {
    use crate::arch::x64::{
        cpuid,
        registers::control::{CR0Flags, CR4Flags, CR0, CR4},
        registers::{msr, RFlags},
        structures::gdt,
    };

    // Set CR0 flags.
    // ### Safety: We set `CR0` once, and setting it again during kernel execution is not supported.
    unsafe { CR0::write(CR0Flags::PE | CR0Flags::MP | CR0Flags::ET | CR0Flags::NE | CR0Flags::WP | CR0Flags::PG) };

    // Set CR4 flags.
    let mut flags = CR4Flags::PAE | CR4Flags::PGE | CR4Flags::OSXMMEXCPT;

    if cpuid::FEATURE_INFO.has_de() {
        flags.insert(CR4Flags::DE);
    }

    if cpuid::FEATURE_INFO.has_fxsave_fxstor() {
        flags.insert(CR4Flags::OSFXSR);
    }

    if cpuid::FEATURE_INFO.has_mce() {
        flags.insert(CR4Flags::MCE);
    }

    if cpuid::FEATURE_INFO.has_pcid() {
        flags.insert(CR4Flags::PCIDE);
    }

    if cpuid::EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_umip) {
        flags.insert(CR4Flags::UMIP);
    }

    if cpuid::EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_fsgsbase) {
        flags.insert(CR4Flags::FSGSBASE);
    }

    if cpuid::EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_smep) {
        flags.insert(CR4Flags::SMEP);
    }

    if cpuid::EXT_FEATURE_INFO.as_ref().map_or(false, cpuid::ExtendedFeatures::has_smap) {
        flags.insert(CR4Flags::SMAP);
    }

    // ### Safety: Initialize the CR4 register with all CPU & kernel supported features.
    unsafe { CR4::write(flags) };

    // Enable use of the `NO_EXECUTE` page attribute, if supported.
    if cpuid::EXT_FUNCTION_INFO.as_ref().map_or(false, cpuid::ExtendedProcessorFeatureIdentifiers::has_execute_disable)
    {
        // ### Safety: Setting `IA32_EFER.NXE` in this context is safe because the bootloader does not use the `NX` bit. However, the kernel does, so
        //         disabling it after paging is in control of the kernel is unsupported.
        unsafe { msr::IA32_EFER::set_nxe(true) };
    } else {
        libcommon::do_once!({
            warn!("PC does not support the NX bit; system security will be compromised (this warning is purely informational).");
        });
    }

    // Load the static processor tables for this core.
    crate::arch::x64::structures::load_static_tables();

    // Setup system call interface.
    // ### Safety: Parameters are set according to the IA-32 SDM, and so should have no undetermined side-effects.
    unsafe {
        // Configure system call environment registers.
        msr::IA32_STAR::set_selectors(gdt::kernel_code_selector().index(), gdt::kernel_data_selector().index());
        msr::IA32_LSTAR::set_syscall({
            /// ### Safety
            ///
            /// This function should never be called by software.
            #[naked]
            unsafe extern "sysv64" fn _syscall_entry() {
                core::arch::asm!(
                    "
                        cld
                        cli                         # always ensure interrupts are disabled within system calls
                        mov rax, rsp                # save the userspace rsp

                        swapgs                      # `swapgs` to switch to kernel stack
                        mov rsp, gs:0x0             # switch to kernel stack
                        swapgs                      # `swapgs` to allow software to use `IA32_KERNEL_GS_BASE` again

                        # preserve registers according to SysV ABI spec
                        push rax    # this pushes the userspace `rsp`
                        push r11    # save usersapce `rflags`
                        push rbx
                        push rbp
                        push r12
                        push r13
                        push r14
                        push r15

                        # push return context as stack arguments
                        push rax
                        push rcx

                        # caller already passed their own arguments in relevant registers
                        call {}

                        pop rcx     # store target `rip` in `rcx`
                        pop rax     # store target `rsp` in `rax`
                        mov [rsp + (7 * 8)], rax   # update userspace `rsp` on stack

                        # restore preserved registers
                        pop r15
                        pop r14
                        pop r13
                        pop r12
                        pop rbp
                        pop rbx
                        pop r11     # restore userspace `rflags`
                        pop rsp     # this restores userspace `rsp`

                        sysretq
                        ",
                    sym syscall_handler,
                    options(noreturn)
                )
            }

            _syscall_entry
        });
        // We don't want to keep any flags set within the syscall (especially the interrupt flag).
        msr::IA32_FMASK::set_rflags_mask(RFlags::all().bits());
        // Enable `syscall`/`sysret`.
        msr::IA32_EFER::set_sce(true);
    }
}

/// ### Safety
///
/// This function should never be called by software.
unsafe extern "sysv64" fn syscall_handler(
    vector: u64,
    arg0: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    arg4: u64,
    ret_ip: u64,
    ret_sp: u64,
    mut syscall_context: crate::cpu::SyscallContext,
) -> crate::cpu::ControlContext {
    // Take a reference to the syscall context, to avoid not mutating the in-memory representation.
    let syscall_context = &mut syscall_context;

    let syscall = match vector {
        0x100 => {
            use log::Level;

            // TODO possibly PR the `log` crate to make `log::Level::from_usize()` public.
            let log_level = match arg0 {
                1 => Ok(Level::Error),
                2 => Ok(Level::Warn),
                3 => Ok(Level::Info),
                4 => Ok(Level::Debug),
                arg0 => Err(arg0),
            };

            match log_level {
                Ok(level) => Some(super::Syscall::Log { level, cstr_ptr: arg1 as usize as *const _ }),
                Err(invalid_level) => {
                    warn!("Invalid log level provided: {}", invalid_level);
                    None
                }
            }
        }

        vector => {
            warn!("Unhandled system call vector: {:#X}", vector);
            None
        }
    };

    match syscall {
        Some(syscall) => super::do_syscall(syscall),
        None => warn!("Failed to execute system call."),
    }

    crate::cpu::ControlContext { ip: ret_ip, sp: ret_sp }
}
