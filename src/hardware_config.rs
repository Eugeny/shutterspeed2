use hal::adc::config::SampleTime;
use hal::pac::SPI1;
use hal::spi::Spi;
use stm32f4xx_hal as hal;

use crate::display::Display;

pub const SAMPLE_TIME: SampleTime = SampleTime::Cycles_3;
pub const SAMPLE_RATE_HZ: u32 = 50_000_u32;
pub const SYSCLK: u32 = 84_000_000;
pub const HCLK: u32 = 42_000_000;
pub const SPI_FREQ_HZ: u32 = 40_000_000;

pub const IPRIO_ADC_TIMER: u8 = 5;

pub type DisplayType = Display<Spi<SPI1>>;

pub struct AllGpio {
    pub a: hal::gpio::gpioa::Parts,
    pub b: hal::gpio::gpiob::Parts,
    pub c: hal::gpio::gpioc::Parts,
}

#[rustfmt::skip]
macro_rules! pin_macro {
    ($d:tt $name:ident, $gpio:ident, $pin:ident) => {
        #[macro_export]
        macro_rules! $name {
            ($d gpio:ident) => {
                $d gpio. $gpio . $pin
            };
        }

        pub use $name;
    };
}

pin_macro!($ display_dc_pin, a, pa8);
pin_macro!($ display_rst_pin, a, pa11);
pin_macro!($ display_sclk_pin, a, pa5);
pin_macro!($ display_miso_pin, a, pa6);
pin_macro!($ display_mosi_pin, a, pa7);
pin_macro!($ display_backlight_pin, b, pb9);

pin_macro!($ adc_pin, a, pa0);

pin_macro!($ led_pin, c, pc13);

pin_macro!($ mode_button_pin, a, pa1);
pin_macro!($ measure_button_pin, a, pa2);

// TIM2 <-> ADC1
// TIM4 -> backlight PWM
// TIM3 -> monotonic
