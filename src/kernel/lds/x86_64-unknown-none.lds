OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_entry)

/* Place kernel in the last 2GB of virtual memory. */
KERN_BASE = 0xffffffff80000000;

PROVIDE(SEGMENT_ALIGN = CONSTANT(MAXPAGESIZE));

SECTIONS
{
    . = KERN_BASE + SIZEOF_HEADERS; 

    .gcc_except_table   : { KEEP(*(.gcc_except_table)) }

    /* eh_frame sections */
    .eh_frame_hdr       : {
        PROVIDE(__eh_frame_hdr = .);
        *(.eh_frame_hdr)
    }
    .eh_frame           : ALIGN(0x8) {
        PROVIDE(__eh_frame = .);
        KEEP(*(.eh_frame .eh_frame.*))
    }

    /* text sections */
    .text               : ALIGN(SEGMENT_ALIGN) {
        PROVIDE(__text_start = .);

        *(.plt) *(.iplt)
        *(.text .text.*)

        PROVIDE(__text_end = .);
    }

    /* rodata sections */
    .rodata             : ALIGN(SEGMENT_ALIGN) {
        PROVIDE(__rodata_start = .);

        *(.rodata .rodata.*)

        PROVIDE(__rodata_end = .);
    }


    .tbss               : { *(.tbss .tbss.*) }
    .tdata              : { *(.tdata .tdata.*) }

    /* relro data */
    .dynamic                : { *(.dynamic) }
    .data.rel.ro        : {
        *(.data.rel.ro.local .data.rel.ro.local.*)
        *(.data.rel.ro .data.rel.ro.*)
    }
    . = DATA_SEGMENT_RELRO_END(0, .);

    /* data sections */
    .data               : ALIGN(SEGMENT_ALIGN) {
        PROVIDE(__data_start = .);

        *(.got) *(.igot)
        *(.got.plt) *(.igot.plt)
        *(.data .data.*)
        *(.limine_reqs)

        PROVIDE(__data_end = .);
    }

    /* bss sections */
    .bss                : ALIGN(SEGMENT_ALIGN) {
        PROVIDE(__bss_start = .);

        *(.dynbss)
        *(.bss .bss.*)

        PROVIDE(__bss_end = .);
    }

    . = DATA_SEGMENT_END(.);

    /* symbols */
    . = ALIGN(SEGMENT_ALIGN);
    PROVIDE(__symbols_start = .);
    .symtab             : { *(.symtab) }
    .strtab             : { *(.strtab) }
    PROVIDE(__symbols_end = .);

    .comment              0 : { *(.comment) }
    .debug                0 : { *(.debug) }
    .debug_abbrev         0 : { *(.debug_abbrev) }
    .debug_aranges        0 : { *(.debug_aranges) }
    .debug_frame          0 : { *(.debug_frame) }
    .debug_funcnames      0 : { *(.debug_funcnames) }
    .debug_info           0 : { *(.debug_info .gnu.linkonce.wi.*) }
    .debug_line           0 : { *(.debug_line) }
    .debug_loc            0 : { *(.debug_loc) }
    .debug_macinfo        0 : { *(.debug_macinfo) }
    .debug_pubnames       0 : { *(.debug_pubnames) }
    .debug_pubtypes       0 : { *(.debug_pubtypes) }
    .debug_ranges         0 : { *(.debug_ranges) }
    .debug_sfnames        0 : { *(.debug_sfnames) }
    .debug_srcinfo        0 : { *(.debug_srcinfo) }
    .debug_str            0 : { *(.debug_str) }
    .debug_typenames      0 : { *(.debug_typenames) }
    .debug_varnames       0 : { *(.debug_varnames) }
    .debug_weaknames      0 : { *(.debug_weaknames) }
    .line                 0 : { *(.line) }
    .shstrtab             0 : { *(.shstrtab) }

}