%macro _save_registers 0
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
%endmacro

%macro _restore_registers 0
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

%macro _save_trace_frame 1
  mov rax, [rsp + ((%1 + 1) * 0)] ; copy the code segment to `rax`

  ; We don't want to try and trace a fault in the kernel back to
  ; userspace, so we check if we're coming from the kernel.
  cmp rax, 0x8 ; are we coming from kernel code?
  je .skip_xor ; if so, don't zero the frame pointer
  xor rbp, rbp ; if not, zero the frame pointer

  .skip_xor:

  ; Copy instruction pointer to `rax`.
  mov rax, [rsp + ((%1 + 1) * 8)]
  
  ; Push the stack frame.
  push rax ; instruction pointer
  push rbp ; previous frame pointer
  mov rbp, rsp
%endmacro

%macro _exception_handler 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_registers
  _save_trace_frame 15

  lea rdi, [rsp + (17 * 8)] ; interrupt stack frame (1st param)
  lea rsi, [rsp + (2 * 8)]  ; saved registers (2nd param)

  call __%1_handler

  add rsp, 0x10 ; pop trace frame

  _restore_registers

  iretq
%endmacro

%macro _exception_handler_with_error 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_registers
  _save_trace_frame 16

  lea rdi, [rsp + (18 * 8)] ; interrupt stack frame (1st param)
  mov rsi, [rsp + (17 * 8)] ; interrupt error code (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; saved registers (3rd param)

  sub rsp, 0x8 ; align stack for sysv calling conv

  call __%1_handler

  add rsp, 0x18 ; pop trace frame & stack alignment

  _restore_registers

  add rsp, 0x8  ; pop interrupt error code

  iretq
%endmacro

%macro _halt_and_catch_fire 0
  .halt_and_catch_fire:
  pause
  jmp .halt_and_catch_fire
%endmacro

%macro _exception_handler_noreturn 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_registers
  _save_trace_frame 15

  lea rdi, [rsp + (17 * 8)] ; interrupt stack frame (1st param)
  lea rsi, [rsp + (2 * 8)]  ; saved registers (2nd param)

  call __%1_handler

  add rsp, 0x10 ; pop trace frame

  _restore_registers

  _halt_and_catch_fire
%endmacro

%macro _exception_handler_with_error_noreturn 1
extern __%1_handler
global __%1_stub
__%1_stub:
  cld

  _save_registers
  _save_trace_frame 16

  lea rdi, [rsp + (18 * 8)] ; interrupt stack frame (1st param)
  mov rsi, [rsp + (17 * 8)] ; interrupt error code (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; saved registers (3rd param)

  sub rsp, 0x8 ; align stack for sysv calling conv

  call __%1_handler

  add rsp, 0x18 ; pop trace frame & stack alignment

  _restore_registers

  add rsp, 0x8  ; pop interrupt error code

  _halt_and_catch_fire
%endmacro

%macro _irq_stub 1
global __irq_%1_stub
__irq_%1_stub:
  cld

  _save_registers
  _save_trace_frame 15

  mov rdi, %1               ; IRQ vector (1st param)
  lea rsi, [rsp + (17 * 8)] ; interrupt stack frame (2nd param)
  lea rdx, [rsp + (2 * 8)]  ; saved registers (3rd param)

  call __irq_handler

  add rsp, 0x10 ; pop trace frame

  _restore_registers

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
_exception_handler mf
_exception_handler xm
_exception_handler ve

_exception_handler_with_error ts
_exception_handler_with_error np
_exception_handler_with_error ss
_exception_handler_with_error gp
_exception_handler_with_error pf
_exception_handler_with_error ac

_exception_handler_noreturn mc

_exception_handler_with_error_noreturn df

extern __irq_handler

%assign irq_number 32
%rep 224
  _irq_stub irq_number
  %assign irq_number irq_number+1
%endrep
