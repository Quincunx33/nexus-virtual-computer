OUTPUT_ARCH(riscv)
ENTRY(_start)

SECTIONS
{
  . = 0x80000000;
  .text : {
    KEEP(*(.text.init))
    *(.text .text.*)
  }
  .rodata : {
    *(.rodata .rodata.*)
    *(.srodata .srodata.*)
  }
  .data : {
    *(.data .data.*)
    *(.sdata .sdata.*)
  }
  .bss (NOLOAD) : {
    *(.bss .bss.*)
    *(.sbss .sbss.*)
    *(COMMON)
  }
  _stack_top = 0x80F00000;
}
