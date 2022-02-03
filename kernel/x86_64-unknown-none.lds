ENTRY(_entry)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
    . = 0x8000; /* 32 KiB */

    .ap_text : ALIGN(0x1000) {
        __ap_text_start = .;

        *(.ap_text.*)
        *(.ap_text)

        __ap_text_end = .;
    }

    .ap_data : ALIGN(0x1000) {
        __ap_data_start = .;

        *(.ap_data.*)
        *(.ap_data)

        __ap_data_end = .;
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

        *(.bss.*)
        *(.bss)

        __bss_end = .;
    }

    .user_code : ALIGN(0x1000) {
        __user_code_start = .;

        *(.user_code.*)
        *(.user_code)

        __user_code_end = .;
    }
}