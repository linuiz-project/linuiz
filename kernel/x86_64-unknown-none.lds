ENTRY(_entry)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
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

    .trampoline : ALIGN(0x1000) {
        __trampoline_start = .;

        *(.trampoline.*)
        *(.trampoline)

        __trampoline_end = .;
    }
}