%macro _halt_and_catch_fire 0
  .halt_and_catch_fire:
  pause
  jmp .halt_and_catch_fire
%endmacro

%macro _save_state 1
  push r15
  push r14
  push r13
  push r12
  push r11
  push r10
  push r9
  push r8
  push rbp
  push rsi
  push rdi
  push rdx
  push rcx
  push rbx
  push rax

  ; We don't want to try and trace a fault in the kernel
  ; back to userspace, so we check if we're coming from the
  ; kernel.
  ;
  ; We do this by reading the stack, prior to the 15
  ; registers we saved, and the instruction pointer from the
  ; interrupt stack frame (16 total qwords).
  cmp qword [rsp + ((16 + %1) * 8)], 0x8 ; coming from kernel code?
  je .skip_zeroing                ; if yes, don't zero trace
  xor rbp, rbp                    ; if no, zero trace
  .skip_zeroing:

  ; Copy instruction pointer to `rax` (qword immediately
  ; prior to the registers we saved).
  mov rax, [rsp + ((15 + %1) * 8)]
  
  ; Push the stack frame.
  push rax ; instruction pointer
  push rbp ; previous frame pointer
  mov rbp, rsp
%endmacro

%macro _restore_state 0
  pop rbp      ; restore previous frame pointer
  add rsp, 0x8 ; pop instruction pointer

  pop rax
  pop rbx
  pop rcx
  pop rdx
  pop rdi
  pop rsi
  pop rbp
  pop r8
  pop r9
  pop r10
  pop r11
  pop r12
  pop r13
  pop r14
  pop r15
%endmacro


%macro _exception_handler 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_state 0             ; adds 17 qwords to stack

  lea rdi, [rsp + (17 * 8)] ; stack frame (1st param)
  lea rsi, [rsp + (2 * 8)]  ; registers (2nd param)

  call __%1_handler         ; call the handler

  _restore_state            ; removes 17 qwords from stack

  iretq
%endmacro

%macro _exception_handler_with_error 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_state 1             ; adds 17 qwords to stack

  lea rdi, [rsp + (18 * 8)] ; stack frame (1st param)
  mov rsi, [rsp + (17 * 8)] ; error code (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; registers (3rd param)

  sub rsp, 0x8              ; align stack for sysv
  call __%1_handler         ; call the handler
  add rsp, 0x8              ; unalign stack for sysv

  _restore_state            ; removes 17 qwords from stack

  iretq
%endmacro

%macro _exception_handler_noreturn 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_state  0            ; adds 17 qwords to stack

  lea rdi, [rsp + (17 * 8)] ; stack frame (1st param)
  lea rsi, [rsp + (2 * 8)]  ; registers (2nd param)

  call __%1_handler         ; call the handler

  _restore_state            ; removes 17 qwords from stack

  _halt_and_catch_fire
%endmacro

%macro _exception_handler_with_error_noreturn 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_state 1             ; adds 17 qwords to stack

  lea rdi, [rsp + (18 * 8)] ; stack frame (1st param)
  mov rsi, [rsp + (17 * 8)] ; error code (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; saved registers (3rd param)

  sub rsp, 0x8              ; align stack for sysv
  call __%1_handler         ; call the handler
  add rsp, 0x8              ; unalign stack for sysv

  _restore_state            ; removes 17 qwords from stack

  _halt_and_catch_fire
%endmacro

%macro _irq_stub 1
global __irq_%1_stub
__irq_%1_stub:
  cld

  _save_state 0             ; adds 17 qwords to stack

  mov rdi, %1               ; IRQ vector (1st param)
  lea rsi, [rsp + (17 * 8)] ; stack frame (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; registers (3rd param)

  call __irq_handler        ; call the handler

  _restore_state            ; removes 17 qwords from stack

  iretq
%endmacro

_exception_handler de
_exception_handler db
_exception_handler nm
_exception_handler bp
_exception_handler of
_exception_handler br
_exception_handler ud
_exception_handler na
_exception_handler_with_error_noreturn df
_exception_handler_with_error ts
_exception_handler_with_error np
_exception_handler_with_error ss
_exception_handler_with_error gp
_exception_handler_with_error pf
_exception_handler mf
_exception_handler_with_error ac
_exception_handler_noreturn mc
_exception_handler xm
_exception_handler ve

extern __irq_handler
%assign irq_number 32
%rep 224
  _irq_stub irq_number
  %assign irq_number irq_number+1
%endrep
