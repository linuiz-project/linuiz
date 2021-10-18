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

    . = 0x200000; /* 2 MiB */
    __ap_stack_bottom = .;

    . = 0x203E80; /* 2 MiB + 16 KiB */
    __ap_stack_top = .;

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

        *(.bss.*)
        *(.bss)

        __bss_end = .;
    }
}