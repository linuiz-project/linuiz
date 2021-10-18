
common __cpu_init_complete 1:1
common __kernel_pml4 8:8
extern __ap_stack_top, _ap_startup 

section .ap_trampoline

bits 16

 _0x8000:
    cli
    cld
    jmp 0:_0x8040

align 16
 _0x8010_GDT_table:
    dq 0, 0
    dq 0x0000FFFF, 0x00CF9A00 ; Flat code
    dq 0x0000FFFF, 0x008F9200 ; Flat data
    dq 0x00000068, 0x00CF8900 ; TSS
 _0x8030_GDT_value:
    dw _0x8030_GDT_value - _0x8010_GDT_table - 1
    dq 0x8010
    dq 0, 0
 
align 64
 _0x8040:
     xor ax, ax
     mov ds, ax
     lgdt [0x8030]
     mov eax, cr0
     or eax, 1
     mov cr0, eax
     jmp 8:_0x8060
 
 bits 32
 
 _0x8060:
     mov ax, 16
     mov ds, ax
     mov ss, ax
     ; Get local APIC ID
     mov eax, 1
     cpuid
     shr ebx, 24
     mov edi, ebx
     ; TODO: Set up 32K stack, one for each cor
     shl ebx, 15
     mov esp, __ap_stack_top
     sub esp, ebx
     push edi
 
 bits 64
 
 ; Wait for BSP to finish initializing core
 bsp_wait:
     pause
     mov ax, [__cpu_init_complete]
     cmp ax, 0
     jz bsp_wait
 
     ; Move kernel PML4 into CR4
     mov r8, [__kernel_pml4]
     mov cr4, r8
     ; Finally, jump to high-level code (never return)
     call _ap_startup
 