ENTRY(_entry)
OUTPUT_FORMAT(elf64-x86-64)

PHDRS {
    ap_text    PT_LOAD FLAGS(0x1);
    ap_data    PT_LOAD FLAGS(0x6);
    text       PT_LOAD FLAGS(0x1);
    rodata     PT_LOAD FLAGS(0x4);
    data       PT_LOAD FLAGS(0x6);
    user_code  PT_LOAD FLAGS(0x1);
}

SECTIONS
{
    . = 0x8000; /* 32 KiB */

    .ap_text : ALIGN(0x1000) {
        PROVIDE(__ap_text_start = .);

        *(.ap_text.* .ap_text)

        PROVIDE(__ap_text_end = .);
    } : ap_text

    .ap_data : ALIGN(0x1000) {
        PROVIDE(__ap_data_start = .);

        *(.ap_data.* .ap_data)

        PROVIDE(__ap_data_end = .);
    } : ap_data

    . = 0x200000; /* 2 MiB */

    .text : ALIGN(0x1000) {
        PROVIDE(__text_start = .);

        *(.text.* .text)

        PROVIDE(__text_end = .);
    } : text

    .rodata : ALIGN(0x1000) {
        PROVIDE(__rodata_start = .);

        *(.rodata.* .rodata)

        PROVIDE(__rodata_end = .);
    } : rodata

    .data : ALIGN(0x1000) {
        PROVIDE(__data_start = .);

        *(.data.* .data)

        PROVIDE(__data_end = .);
    } : data

    .bss : ALIGN(0x1000) {
        PROVIDE(__bss_start = .);

        . += 0x2000;
        __bsp_stack = .;

        . += 0x1000;
        __syscall_stack = .;


        *(.bss.* .bss)

        PROVIDE(__bss_end = .);
    } : data

    .user_code : ALIGN(0x1000) {
        PROVIDE(__user_code_start = .);

        *(.user_code.* .user_code)

        PROVIDE(__user_code_end = .);
    }
}