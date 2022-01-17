ENTRY(_entry)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
    . = 0x8000; /* 32 KiB */

    .ap_trampoline : ALIGN(0x40) {
        __ap_trampoline_start = .;

        *(.ap_trampoline.*)
        *(.ap_trampoline)

        __ap_trampoline_end = .;
    }

    .global_data : ALIGN(0x40) {
        __global_data_start = .;

        *(.global_data.*)
        *(.global_data)

        __global_data_end = .;
    }

    . = 0x200000; /* 2 MiB */

    .text : ALIGN(0x1000) {
        __text_start = .;

        *(.text.*)
        *(.text)

        __text_end = .;
    }

    .rodata : ALIGN(0x1000) {
        __rodata_start = .;

        *(.rodata.*)
        *(.rodata)

        __rodata_end = .;
    }

    .data : ALIGN(0x1000) {
        __data_start = .;

        *(.data.*)
        *(.data)

        __data_end = .;
    }

    .bss : ALIGN(0x1000) {
        __bss_start = .;

        __bsp_stack_bottom = .;
        __bsp_stack_top = . + 0x4000;

        *(.bss.*)
        *(.bss)

        __bss_end = .;
    }
}