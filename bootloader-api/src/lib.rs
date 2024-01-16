#![no_std]

extern "C" {
    static mut BOOTLOADER_FLAGS: u32;
    static mut APP_START: u32;
}

const FLAG_REBOOT_DFU: u32 = 0x5AA55AA5;

#[allow(clippy::missing_safety_doc)]
pub unsafe fn app_ptr() -> *const u32 {
    core::ptr::addr_of!(APP_START)
}

fn read_flag() -> u32 {
    unsafe { core::ptr::read_volatile(&BOOTLOADER_FLAGS) }
}

fn write_flag(flags: u32) {
    unsafe { core::ptr::write_volatile(&mut BOOTLOADER_FLAGS, flags) }
}

pub fn reset_bootloader_flags() {
    write_flag(0);
}

pub fn is_dfu_boot_flag_set() -> bool {
    read_flag() == FLAG_REBOOT_DFU
}

pub fn reboot_into_bootloader() -> ! {
    write_flag(FLAG_REBOOT_DFU);
    cortex_m::interrupt::disable();
    cortex_m::peripheral::SCB::sys_reset()
}
