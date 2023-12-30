#![no_main]
#![no_std]

use cortex_m_rt::entry;
use hal::gpio::GpioExt;
use {panic_halt as _, stm32f4xx_hal as hal};

use crate::hal::pac;

#[entry]
fn main() -> ! {
    let p = pac::Peripherals::take().unwrap();
    let gpioc = p.GPIOC.split();
    let mut led = gpioc.pc13.into_push_pull_output();
    led.set_low();

    let is_dfu_boot = bootloader_api::is_dfu_boot_flag_set();
    bootloader_api::reset_bootloader_flags();

    if is_dfu_boot {
        jump_to_bootloader()
    }

    for _ in 0..100000 {
        led.set_high();
    }
    for _ in 0..100000 {
        led.set_low();
    }

    jump_to_app();
}

fn jump_to_bootloader() -> ! {
    unsafe {
        cortex_m::interrupt::enable();
        cortex_m::asm::bootload(0x1FFF0000 as *const u32)
    }
}

fn jump_to_app() -> ! {
    unsafe {
        cortex_m::interrupt::enable();
        cortex_m::asm::bootload(0x00004000 as *const u32)
    }
}
