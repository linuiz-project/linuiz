
common __cpu_init_complete 1:1
common __kernel_pml4 8:8

section .ap_trampoline

bits 16

; %define CODE_SEG    0x0008
; %define DATA_SEG    0x0010

; align 4
; IDT:
;     len     dw 0
;     base    dw 0

; GDT:
; .null    dq 0x0
; .code    dq 0x00209A0000000000, 0x0000920000000000
;         dw 0

; .pointer:
;         dw $ - GDT - 1
;         dd GDT

; ; Function for switching directly to long mode from real mode.
; stack dw 0
; elevate_long_mode:
;     mov esp, $stack
;     push di         ; `rep stosd` alters `di`
;     mov ecx, 0x1000
;     xor eax, eax
;     cld
;     rep stosd
;     pop di          ; retrieve di

;     ; Disable IRQs.
;     mov al, 0xFF
;     out 0xA1, al
;     out 0x21, al

;     ; Wait to ensure ports are clear.
;     nop
;     nop

;     lidt [IDT]         ; Load zero-length IDT

;     ; Enter long mode.
;     mov eax, 10100000b          ; Set PAE & PGE bits.
;     mov cr4, eax

;     mov edx, [__kernel_pml4]
;     mov cr3, edx                ; Set PML4 in CR3.

;     mov ecx, 0xC0000080         ; Read from EFER MSR.
;     rdmsr

;     or eax, 0x00000100          ; Set LME bit.
;     wrmsr

;     mov ebx, cr0                ; Activate long mode—
;     or ebx, 0x80000001         ;   —by enabling paging and protection simultaneously.
;     mov cr0, ebx

;     lgdt [GDT.pointer]

;     jmp CODE_SEG:long_mode

; bits 64
; extern ap_startup
; long_mode:
;     mov ax, DATA_SEG
;     mov ds, ax
;     mov es, ax
;     mov fs, ax
;     mov gs, ax
;     mov ss, ax

;     call ap_startup
;     ; Does not return

 _0x8000:
     cli
     cld
     jmp 0:0x8040
 
 _0x8010_GDT_table:
     dq 0, 0
     dq 0x0000FFFF, 0x00CF9A00 ; Flat code
     dq 0x0000FFFF, 0x008F9200 ; Flat data
     dq 0x00000068, 0x00CF8900 ; TSS
 _0x8030_GDT_value:
     dw _0x8030_GDT_value - _0x8010_GDT_table - 1
     dq 0x8010
     dq 0, 0
 
 _0x8040:
     xor ax, ax
     mov ds, ax
     lgdt [0x8030]
     mov eax, cr0
     or eax, 1
     mov cr0, eax
     jmp 8:0x8060
 
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
     mov esp, 1      ; TODO stack_top instead of `1`
     sub esp, ebx
     push edi
 
 bits 64
 extern ap_startup 
 
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
     call ap_startup
 