#![no_std]

pub use {display_interface_spi, embedded_time, fugit, stm32f4xx_hal as hal};

#[macro_use]
mod macros;

// Timer allocation
// TIM2 <-> ADC1
// TIM3 -> display delay
// TIM4 -> sound PWM

pub const CALIBRATION_TIME_MS: u32 = 1000;
pub const TRIGGER_THRESHOLD_LOW: f32 = 1.3;
pub const TRIGGER_THRESHOLD_HIGH: f32 = 1.5;
pub const ADC_RESOLUTION: Resolution = Resolution::Eight;

pub const SAMPLE_TIME: SampleTime = SampleTime::Cycles_3;
pub const SAMPLE_RATE_HZ: u32 = 100_000_u32;
pub const SYSCLK: u32 = 84_000_000;
pub const HCLK: u32 = 42_000_000;
pub const SPI_FREQ_HZ: u32 = 10_000_000;

pub type DisplaySpiType = Spi<SPI1>;
pub type DmaTransfer = Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut u16>;
pub type AdcTimerType = CounterHz<TIM2>;

#[macro_export]
macro_rules! setup_clocks {
    ($dp:expr) => {{
        use $crate::hal::prelude::*;
        let rcc = $dp.RCC.constrain();
        rcc.cfgr
            .sysclk($crate::SYSCLK.Hz())
            .require_pll48clk()
            .hclk($crate::HCLK.MHz())
            .use_hse(25.MHz())
            .pclk1(80.MHz())
            .pclk2(80.MHz())
            .freeze()
    }};
}

pub fn _setup_adc_timer(t: TIM2, clocks: &Clocks) -> CounterHz<TIM2> {
    use hal::timer::Event;

    let mut timer = t.counter_hz(clocks);
    timer.listen(Event::Update);
    timer.start(SAMPLE_RATE_HZ.Hz()).unwrap();

    timer
}
#[macro_export]
macro_rules! setup_adc_timer {
    ($dp:expr, $clocks:expr) => {{
        $crate::_setup_adc_timer($dp.TIM2, $clocks)
    }};
}

pub fn _setup_adc(adc: ADC1, adc_pin: Pin<'A', 0, Analog>) -> Adc<ADC1> {
    use hal::adc::config::{AdcConfig, Clock, Scan, Sequence};

    let adc_config = AdcConfig::default()
        .dma(Dma::Continuous)
        .scan(Scan::Disabled)
        .clock(Clock::Pclk2_div_6)
        .resolution(ADC_RESOLUTION);

    let mut adc = Adc::adc1(adc, true, adc_config);
    adc.configure_channel(&adc_pin, Sequence::One, SAMPLE_TIME);
    adc
}

#[macro_export]
macro_rules! setup_adc {
    ($dp:expr, $gpio:expr) => {{
        $crate::_setup_adc($dp.ADC1, $gpio.a.pa0.into_analog())
    }};
}

#[macro_export]
macro_rules! setup_adc_dma_transfer {
    ($core:expr, $dp:expr, $adc:expr, $buffer:expr) => {{
        use hal::dma::config::DmaConfig;
        use hal::dma::{PeripheralToMemory, Stream0, StreamsTuple, Transfer};

        let dma = StreamsTuple::new($dp.DMA2);
        let dma_config = DmaConfig::default()
            .transfer_complete_interrupt(true)
            .double_buffer(false);

        Transfer::init_peripheral_to_memory(dma.0, $adc, $buffer, None, dma_config)
    }};
}

#[macro_export]
macro_rules! delay_timer {
    ($dp:expr) => {
        $dp.TIM3
    };
}

#[macro_export]
macro_rules! setup_display_spi {
    ($dp:expr, $gpio:expr, $clocks:expr) => {{
        use $crate::fugit::RateExtU32;
        use $crate::hal::spi::Spi;

        let mut sclk_pin = hw::display_sclk_pin!($gpio).into_alternate();
        let mut miso_pin = hw::display_miso_pin!($gpio).into_alternate();
        let mut mosi_pin = hw::display_mosi_pin!($gpio).into_alternate();
        sclk_pin.set_speed(Speed::VeryHigh);
        miso_pin.set_speed(Speed::VeryHigh);
        mosi_pin.set_speed(Speed::VeryHigh);

        Spi::new(
            $dp.SPI1,
            (sclk_pin, miso_pin, mosi_pin),
            embedded_hal::spi::MODE_3,
            $crate::SPI_FREQ_HZ.Hz(),
            &$clocks,
        )
    }};
}

#[macro_export]
macro_rules! setup_display {
    ($dp:expr, $gpio:expr, $clocks:expr, $delay:expr) => {{
        use $crate::display_interface_spi::SPIInterfaceNoCS;
        use $crate::hal::gpio::{Edge, ErasedPin, Input, Output, Speed};
        let spi = $crate::setup_display_spi!($dp, $gpio, $clocks);
        let mut dc_pin = $crate::display_dc_pin!($gpio).into_push_pull_output();
        let mut rst_pin = $crate::display_rst_pin!($gpio).into_push_pull_output();
        dc_pin.set_speed(Speed::VeryHigh);
        rst_pin.set_speed(Speed::VeryHigh);

        let di = SPIInterfaceNoCS::new(spi, dc_pin.erase());
        mipidsi::Builder::st7735s(di)
            .with_orientation(mipidsi::Orientation::Portrait(false))
            .with_invert_colors(mipidsi::ColorInversion::Normal)
            .with_display_size(128, 160)
            .init($delay, Some(rst_pin.erase()))
    }};
}

#[macro_export]
macro_rules! beeper_type {
    () => {
        use embedded_time::rate::Hertz;
        use $crate::hal::pac::TIM4;
        use $crate::hal::timer::{ChannelBuilder, PwmHz};

        pub struct Beeper {
            pwm: PwmHz<TIM4, ChannelBuilder<TIM4, 2>>,
        }

        impl BeeperExt for Beeper {
            fn enable(&mut self, frequency: f32) {
                use hal::timer::Channel;

                self.pwm.set_period((frequency as u32).Hz());
                self.pwm.enable(Channel::C3);
            }

            fn disable(&mut self) {
                use hal::timer::Channel;
                self.pwm.set_period(10.Hz());
                self.pwm.enable(Channel::C3);
                self.pwm.disable(Channel::C3);
            }

            fn set_duty_percent(&mut self, duty_percent: u8) {
                use hal::timer::Channel;
                self.pwm.set_duty(
                    Channel::C3,
                    (self.pwm.get_max_duty() * duty_percent as u16) / 100,
                );
            }
        }
    };
}

#[macro_export]
macro_rules! setup_sound_pwm {
    ($dp:expr, $gpio:expr, $clocks:expr) => {{
        use hal::timer::Channel;

        let mut buzzer_pin = $gpio.b.pb8.into_alternate();
        let ch = hal::timer::pwm::Channel3::new(buzzer_pin);
        let mut pwm = $dp.TIM4.pwm_hz(ch, 550.Hz(), $clocks);
        pwm.set_duty(Channel::C3, pwm.get_max_duty() / 2);

        Beeper { pwm }
    }};
}

pub struct AllGpio {
    pub a: hal::gpio::gpioa::Parts,
    pub b: hal::gpio::gpiob::Parts,
    pub c: hal::gpio::gpioc::Parts,
}

pin_macro!($ display_dc_pin, a, pa8);
pin_macro!($ display_rst_pin, b, pb5);
pin_macro!($ display_sclk_pin, a, pa5);
pin_macro!($ display_miso_pin, a, pa6);
pin_macro!($ display_mosi_pin, a, pa7);
pin_macro!($ display_backlight_pin, b, pb9);

pin_macro!($ adc_pin, a, pa0);

pin_macro!($ led_pin, c, pc13);

pin_macro!($ measure_button_pin, a, pa2);

pin_macro!($ usb_dm_pin, a, pa11);
pin_macro!($ usb_dp_pin, a, pa12);

pin_macro!($ rotary_dt_pin, c, pc14);
pin_macro!($ rotary_clk_pin, c, pc15);

use fugit::RateExtU32;
use hal::adc::config::{Dma, Resolution, SampleTime};
use hal::adc::Adc;
use hal::dma::{PeripheralToMemory, Stream0, Transfer};
use hal::gpio::{Analog, Pin};
use hal::pac::{ADC1, DMA2, SPI1, TIM2};
use hal::rcc::Clocks;
use hal::spi::Spi;
use hal::timer::{CounterHz, TimerExt};
use hal::Listen;
