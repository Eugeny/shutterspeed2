MEMORY
{
  /* NOTE 1 K = 1 KiBi = 1024 bytes */
  /* TODO Adjust these memory regions to match your device memory layout */
  /* These values correspond to the LM3S6965, one of the few devices QEMU can emulate */

  /* actual flash size is 256K / 0x40000 */
  /* leave 16K for the bootloader */

  /* FLASH : ORIGIN = 0x00000000, LENGTH = 192K */

  BOOTLOADER_RESERVED : ORIGIN = 0x00000000, LENGTH = 16K
  FLASH : ORIGIN = 0x00004000, LENGTH = 240K
  RAM : ORIGIN = 0x20000000, LENGTH = 62K /* 2 KB left free for heap and bootloader flags */
  HEAP : ORIGIN = 0x2000F800, LENGTH = 1K
}

SECTIONS {
   .bootloader 0x0 :
   {
      KEEP(*(.bootloaderReservedSection))
      /* . = 0x0FFFC; */
      LONG(0xBEEFDEAD)
   } > BOOTLOADER_RESERVED

   .heap ORIGIN(HEAP) :
   {
      HEAP = ORIGIN(HEAP);
   } > HEAP
}

INCLUDE ../../../../../../bootloader-api/link.x

/* This is where the call stack will be allocated. */
/* The stack is of the full descending type. */
/* You may want to use this variable to locate the call stack and static
   variables in different memory regions. Below is shown the default value */
/* _stack_start = ORIGIN(RAM) + LENGTH(RAM); */

/* You can use this symbol to customize the location of the .text section */
/* If omitted the .text section will be placed right after the .vector_table
   section */
/* This is required only on microcontrollers that store some configuration right
   after the vector table */
/* _stext = ORIGIN(FLASH) + 0x400; */

/* Example of putting non-initialized variables into custom RAM locations. */
/* This assumes you have defined a region RAM2 above, and in the Rust
   sources added the attribute `#[link_section = ".ram2bss"]` to the data
   you want to place there. */
/* Note that the section will not be zero-initialized by the runtime! */
/* SECTIONS {
     .ram2bss (NOLOAD) : ALIGN(4) {
       *(.ram2bss);
       . = ALIGN(4);
     } > RAM2
   } INSERT AFTER .bss;
*/
