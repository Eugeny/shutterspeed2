#![no_main]
#![no_std]

use core::fmt::Debug;

use cortex_m_rt::entry;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use hal::gpio::GpioExt;
use hal::timer::TimerExt;
use u8g2_fonts::fonts::u8g2_font_profont17_mr;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::FontRenderer;
use {config as hw, panic_abort as _, stm32f4xx_hal as hal};

use crate::hal::pac;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let gpio = config::AllGpio {
        a: dp.GPIOA.split(),
        b: dp.GPIOB.split(),
        c: dp.GPIOC.split(),
    };

    let mut led = gpio.c.pc13.into_push_pull_output();
    led.set_low();

    let is_dfu_boot = bootloader_api::is_dfu_boot_flag_set();
    bootloader_api::reset_bootloader_flags();

    if is_dfu_boot {
        let clocks = config::setup_clocks!(dp);
        let mut delay = config::delay_timer!(dp).delay_us(&clocks);
        let mut display =
            unsafe { hw::setup_display!(dp, gpio, &clocks, &mut delay).unwrap_unchecked() };
        hw::display_backlight_pin!(gpio)
            .into_push_pull_output()
            .set_high();

        dfu(&mut display);
    }

    for _ in 0..100000 {
        led.set_high();
    }
    for _ in 0..100000 {
        led.set_low();
    }

    jump_to_app();
}

pub const FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont17_mr>();

fn dfu<D: DrawTarget<Color = Rgb565, Error = E>, E: Debug>(display: &mut D) -> ! {
    let _ = display.clear(Rgb565::RED);
    let p = display.bounding_box().center() - Point::new(0, 40);

    let _ = FONT.render_aligned(
        " DFU MODE ",
        p,
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            bg: Rgb565::BLACK,
            fg: Rgb565::RED,
        },
        display,
    );
    let _ = FONT.render_aligned(
        " READY TO ",
        p + Point::new(0, 30),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            fg: Rgb565::BLACK,
            bg: Rgb565::RED,
        },
        display,
    );
    let _ = FONT.render_aligned(
        " RECEIVE ",
        p + Point::new(0, 45),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            fg: Rgb565::BLACK,
            bg: Rgb565::RED,
        },
        display,
    );
    let _ = FONT.render_aligned(
        " UPDATES ",
        p + Point::new(0, 60),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            fg: Rgb565::BLACK,
            bg: Rgb565::RED,
        },
        display,
    );
    jump_to_bootloader()
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
        cortex_m::asm::bootload(bootloader_api::app_ptr())
    }
}
