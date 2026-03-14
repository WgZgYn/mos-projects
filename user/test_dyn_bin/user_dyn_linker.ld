ENTRY(_start)

PHDRS
{
  text    PT_LOAD FLAGS(5);  /* R X */
  rodata  PT_LOAD FLAGS(4);  /* R   */
  data    PT_LOAD FLAGS(6);  /* R W */
  dynamic PT_DYNAMIC FLAGS(6);
}

SECTIONS
{
  . = 0x400000;

  /* ---------------- RX ---------------- */

  .text : ALIGN(4K)
  {
    *(.text .text.*)
    *(.plt)
  } :text

  /* ---------------- R ---------------- */

  . = ALIGN(4K);

  .rodata :
  {
    *(.rodata .rodata.*)
    *(.eh_frame*)
  } :rodata

  .rela.dyn :
  {
    *(.rela.dyn)
  } :rodata

  .rela.plt :
  {
    *(.rela.plt)
  } :rodata

  /* ---------------- RW ---------------- */

  . = ALIGN(4K);

  .data :
  {
    *(.data .data.*)
  } :data

  .got :
  {
    *(.got)
  } :data

  .got.plt :
  {
    *(.got.plt)
  } :data

  .dynamic :
  {
    *(.dynamic)
  } :data :dynamic

  .bss (NOLOAD) :
  {
    *(.bss .bss.*)
    *(COMMON)
  } :data
}