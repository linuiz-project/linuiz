OUTPUT_FORMAT(elf64-x86-64)
OUTPUT_ARCH(i386:x86-64)

ENTRY(_entry)

/* Place kernel in the last 2GB of virtual memory. */
KERN_BASE = 0xffffffff80000000;

SECTIONS
{
    PROVIDE(__SECTION_ALIGN =  0x200000);

    . = KERN_BASE + SIZEOF_HEADERS; 

    .hash               : { *(.hash) }
    .gnu.hash           : { *(.gnu.hash) }
    .dynsym             : { *(.dynsym) }
    .dynstr             : { *(.dynstr) }
    .rela.dyn           :
    {
        *(.rela.dyn)
        *(.rela.text    .rela.text.*)
        *(.rela.rodata  .rela.rodata.*)
        *(.rela.data.rel.ro*)
        *(.rela.got)
        *(.rela.data    .rela.data.*)
        *(.rela.tdata   .rela.tdata.*)
        *(.rela.tbss    .rela.tbss.*)
        *(.rela.bss     .rela.bss.*)
        *(.rela.ifunc)
    }
    .rela.plt           : { *(.rela.plt) *(.rela.iplt) }

    /* text sections */
    .text               : ALIGN(__SECTION_ALIGN) {
        PROVIDE(__text_start = .);

        *(.plt) *(.iplt)
        *(.text .text.*)
    
        PROVIDE(__text_end = .);
    }
    
    /* rodata sections */
    .rodata             : ALIGN(__SECTION_ALIGN) {
        PROVIDE(__rodata_start = .);

        *(.rodata .rodata.*)
        *(.eh_frame_hdr)

        PROVIDE(__rodata_end = .);
    }
    
    /* bss sections */
    .bss                : ALIGN(__SECTION_ALIGN) {
        PROVIDE(__bss_start = .);

        KEEP(*(.eh_frame .eh_frame.*))
        KEEP(*(.gcc_except_table))
        *(.tdata .tdata.*)
        *(.tbss .tbss.*)
        *(.data.rel.ro.local .data.rel.ro.local.*)
        *(.data.rel.ro .data.rel.ro.*)
        *(.dynamic)
        *(.got) *(.igot)
        *(.bss .bss.*)

        PROVIDE(__bss_end = .);
    }

    /* data sections */
    .data               : ALIGN(__SECTION_ALIGN) {
        PROVIDE(__data_start = .);

        *(.got.plt) *(.igot.plt)
        *(.data .data.*)
        *(.limine_reqs)

        PROVIDE(__data_end = .);
    }

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
    .strtab               0 : { *(.strtab) }
    .symtab               0 : { *(.symtab) }
}