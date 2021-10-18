
common __cpu_init_complete 1:1
common __kernel_pml4 4:4
extern __ap_stack_top, _ap_startup 

section .ap_trampoline

bits 16

realmode:
    cli
    cld

    ; Enable PAE
    mov eax, cr4
    or eax, 1 << 5
    mov cr4, eax

    ; Set PML4 address
    or eax, __kernel_pml4

    ; Enable long mode
    mov ecx, 0xC0000080     ; Set correct MSR
    rdmsr
    or eax, 1 << 8          ; Set LME bit
    wrmsr

    ; Set PME & PGE bits
    mov eax, cr0
    or eax, 1 << 31 | 1 << 0
    mov cr0, eax

bits 32

    ; Set GDT, prepare stack, long-jump to long mode
    mov esp, [__ap_stack_top]
    lgdt [GDT.pointer]
    jmp GDT.code:longmode


bits 64

longmode:
    ; Clear segment registers
    mov ax, GDT.data    
    mov ds, ax
    mov es, ax
    mov fs, ax
    mov gs, ax
    mov ss, ax
    mov ax, GDT.code
    mov cs, ax

    call _ap_startup


GDT:
    .null: equ $ - GDT
        dq 0           
    .code: equ $ - GDT
        dw 0            
        dw 0xFFFF            
        db 0            
        db 1011010b    
        db 1110111b    
        db 0              
    .data: equ $ - GDT
        dw 0            
        dw 0xFFFF            
        db 0            
        db 1010010b    
        db 1110111b     
        db 0            
    .tss: equ $ - GDT
        dd 0x00000068
        dd 0x00CF8900
    .pointer:
        dw $ - GDT - 1
        dq GDT
