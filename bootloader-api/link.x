MEMORY
{
  BOOTLOADER_FLAGS : ORIGIN = 0x2000FC00, LENGTH = 1K
}

SECTIONS {
   .bootloader_flags ORIGIN(BOOTLOADER_FLAGS) :
   {
      BOOTLOADER_FLAGS = ORIGIN(BOOTLOADER_FLAGS);
   } > BOOTLOADER_FLAGS
}