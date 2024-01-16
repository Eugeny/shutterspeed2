MEMORY
{
  FLASH : ORIGIN = 0x00000000, LENGTH = 16K
  APP : ORIGIN = ORIGIN(FLASH) + LENGTH(FLASH), LENGTH = 1K
  RAM : ORIGIN = 0x20000000, LENGTH = 63K /* last KB left free for bootloader flags */
}

SECTIONS {
   .app_start ORIGIN(APP) :
   {
      APP_START = ORIGIN(APP);
   } > APP
}


INCLUDE ../../../../../../bootloader-api/link.x
