ENTRY(_start)
OUTPUT_FORMAT("elf64-x86-64")
SECTIONS
{
  /* 将程序链接到 0x0040_0000 */
  . = 0x400000;
  .text : {
    *(.text*)
    *(.rodata*)
  }
  .data : { *(.data*) }
  .bss  : { *(.bss*) *(COMMON) }
  /DISCARD/ : { *(.eh_frame) }
}
