OUTPUT_ARCH(riscv64gc)

ENTRY(_boot)

SECTIONS {
    . = 0x80200000;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __kernel_start      = .;

    /* TEXT */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __text_start        = .;
    .text               : { *(.init.boot) *(.text .text.*) }
    __text_end          = .;


    /* RODATA */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __rodata_start      = .;
    .rodata             : { *(.rodata .rodata.*) }
    __rodata_end        = .;


    /* DATA */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __data_start        = .;
    .data               : { *(.data .data.*) }

    . = ALIGN(8);
    __global_pointer$   = .;
    .sdata              : { *(.sdata .sdata.*) }

    __data_end          = .;


    /* BSS */
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __bss_start         = .;
    .bss                : { *(.bss .bss.*) }
    .sbss               : { *(.sbss .sbss.*) }
    __bss_end           = .;


    . = ALIGN(CONSTANT(MAXPAGESIZE));
    . += 8 * 0x1000;
    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __boot_stack_top    = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __kernel_end        = .;
    
    /DISCARD/ : { *(.eh_frame_hdr .eh_frame) }
}