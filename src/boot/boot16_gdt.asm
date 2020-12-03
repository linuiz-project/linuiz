; below is the specific GDT required to elevate from
; 16-bit real mode to 32-bit protected mode
;
; it is in a separate file to help isolate it, and ensure
; that its code isn't modified without express intent

gdt_start:

gdt_null:   ; mandatory null descriptor
    dd 0x0  ; dd is a double-word (4 bytes)
    dd 0x0

gdt_code:   ; code segment descriptor
    ; base=0x0, limit=0xFFFFFF,
    ; 1st flags: present=1, privilege=0, descriptor type=1              (bits 1001b)
    ; type flags: code=1, conforming=0, readable=1, accessed=0          (bits 1010b)
    ; 2nd flags: granularity=1, 32-bit default=1, 64-bit seg=0, AVL=0   (bits 1100b)
    dw 0xFFFF       ; Limit (bits 0-15)
    dw 0x0          ; Base (bits 0-15)
    db 0x0          ; Base (bits 16-23)
    db 10011010b    ; 1st flags, type flags
    db 11001111b    ; 2nd flags, Limit (bits 16-19)
    db 0x0          ; Base (bits 24-31)

gdt_data:   ; data segment descriptor
    ; Same as code segment, except:
    ; type flags: code=0, expand down=0, writable=1, accessed=0 (bits 0010b)
    dw 0xFFFF       ; Limit (bits 0-15)
    dw 0x0          ; Base (bits 0-15)
    db 0x0          ; Base (bits 16-23)
    db 10010010b    ; 1st flags, type flags
    db 11001111b    ; 2nd flags, Limit (bits 16-19)
    db 0x0          ; Base (bits 24-31)


; this label is here to let the assembler 
; calculate the size of the GDT (below)
gdt_end:


gdt_descriptor:     ; gdt descriptor
    dw gdt_end - gdt_start - 1  ; size of our GDT, always less one of the true size
    dd gdt_start                ; start of our GDT


; Define some handy constants for the GDT segment descriptor offsets, which
; are what segment registers must contain when in protected mode. For example,
; when we set `ds`=0x10 in protected mode, the CPU knows that we mean it to use the
; segment described at offset 0x10 (i.e. 16  bytes) in our GDT, which in our
; case is the DATA segment (0x0 -> NULL; 0x08 -> CODE; 0x10 -> DATA)
code_segment equ gdt_code - gdt_start
data_segment equ gdt_data - gdt_start