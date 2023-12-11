#![deny(unsafe_code)]
#![allow(clippy::empty_loop)]
#![no_main]
#![no_std]

mod display;
mod util;

use display::Display;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_hal::blocking::delay::DelayMs;
use hal::adc::config::{AdcConfig, Resolution, SampleTime, Dma, Sequence};
use hal::gpio::Speed;
use hal::spi::Spi;
use panic_halt as _;

use cortex_m_rt::entry;
use stm32f4xx_hal as hal;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use u8g2_fonts::FontRenderer;
use ufmt::uwrite;

use crate::hal::{pac, prelude::*};

const DISPLAY_BRIGHTNESS: f32 = 0.1;
const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen16x32_me>();
const DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen32x64_mn>();
const SAMPLE_TIME: SampleTime = SampleTime::Cycles_112;

#[entry]
fn main() -> ! {
    let dp = pac::Peripherals::take().unwrap();
    let cp = cortex_m::peripheral::Peripherals::take().unwrap();

    let gpioa = dp.GPIOA.split();
    let gpiob = dp.GPIOB.split();

    // Set up the system clock. We want to run at 48MHz for this one.
    dp.RCC.apb2enr.write(|w| w.syscfgen().enabled());
    let rcc = dp.RCC.constrain();
    let clocks = rcc
        .cfgr
        .hclk(84.MHz())
        // .use_hse(25.MHz())
        .sysclk(48.MHz())
        .freeze();

    let mut delay = dp.TIM1.delay_us(&clocks);

    // -----------

    let mut pwm = dp
        .TIM4
        .pwm_hz(hal::timer::Channel4::new(gpiob.pb9), 100.Hz(), &clocks);
    pwm.enable(hal::timer::Channel::C4);
    pwm.set_duty(hal::timer::Channel::C4, 0);

    let mut display = {
        let mut dc_pin = gpioa.pa8.into_push_pull_output();
        let mut rst_pin = gpioa.pa10.into_push_pull_output();
        let mut sclk_pin = gpioa.pa5.into_alternate();
        let mut miso_pin = gpioa.pa6.into_alternate();
        let mut mosi_pin = gpioa.pa7.into_alternate();
        dc_pin.set_speed(Speed::VeryHigh);
        rst_pin.set_speed(Speed::VeryHigh);
        sclk_pin.set_speed(Speed::VeryHigh);
        miso_pin.set_speed(Speed::VeryHigh);
        mosi_pin.set_speed(Speed::VeryHigh);
        let spi = Spi::new(
            dp.SPI1,
            (sclk_pin, miso_pin, mosi_pin),
            embedded_hal::spi::MODE_3,
            2.MHz(),
            &clocks,
        );
        let mut display = Display::new(spi, dc_pin, rst_pin, &mut delay);
        display.clear();
        display
    };

    pwm.set_duty(
        hal::timer::Channel::C4,
        (pwm.get_max_duty() as f32 * DISPLAY_BRIGHTNESS) as u16,
    );

    let adc_pin = gpioa.pa0.into_analog();
    let mut adc = hal::adc::Adc::adc1(
        dp.ADC1,
        true,
        AdcConfig::default()
            // .dma(Dma::Continuous)
            .resolution(Resolution::Twelve)
            .default_sample_time(SAMPLE_TIME)
            .continuous(hal::adc::config::Continuous::Continuous),
    );
    // adc.configure_channel(&adc_pin, Sequence::One, SAMPLE_TIME);
    adc.start_conversion();

    let gpioc = dp.GPIOC.split();
    let mut led_pin = gpioc.pc13.into_push_pull_output();
    let mode_button_pin = gpioa.pa1.into_pull_up_input();

    let mut s = util::EString::<128>::default();

    /*
    u8g2_font_profont29_mf
    u8g2_font_spleen16x32_me
     */

    loop {
        s.clear();

        // let value = adc.current_sample();
        let value =adc.convert(&adc_pin, SAMPLE_TIME);

        let _ = uwrite!(s, "{}  ", value);

        let res = TEXT_FONT.render(
            "Current value:",
            Point::new(50, 50),
            VerticalPosition::Top,
            // FontColor::Transparent( Rgb565::RED),
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut *display,
        );
        if let Err(err) = res {
            s.clear();
            use core::fmt::Write;
            let _ = write!(*s, "Failed with: {:?}", err);
            display.panic_error(&s[..]);
        }

        let res = DIGIT_FONT.render(
            &s[..],
            Point::new(50, 100),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut *display,
        );
        if let Err(err) = res {
            s.clear();
            use core::fmt::Write;
            let _ = write!(*s, "Failed with: {:?}", err);
            display.panic_error(&s[..]);
        }

        // loop {
        // On for 1s, off for 3s.
        led_pin.set_high();
        // Use `embedded_hal::DelayMs` trait
        delay.delay_ms(20_u32);
        led_pin.set_low();
        // or use `fugit::ExtU32` trait
        delay.delay_ms(20_u32);

        if mode_button_pin.is_low() {
            // counter = 0;
        }
    }
}
