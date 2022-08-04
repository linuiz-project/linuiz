#[naked]
#[no_mangle]
#[allow(named_asm_labels)]
#[link_section = ".init.boot"]
pub unsafe extern "C" fn _boot() -> ! {
    core::arch::asm!(
        "
        csrw sie, zero
        csrci sstatus, 2

        .option push
        .option norelax
        lla gp, __global_pointer$
        .option pop

        lla t0, __bss_start
        lla t1, __bss_end

        # `.bss` needs to be zeroed to conform to standard access
        _clear_bss:
            beq t0, t1, _done_clear_bss
            sd zero, (t0)
            addi t0, t0, 8
            j _clear_bss

        _done_clear_bss:

        lla sp, __boot_stack_top

        j _test
        ",
        options(noreturn)
    )
}
