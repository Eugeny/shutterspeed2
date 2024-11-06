#![no_std]

pub use {display_interface_spi, embedded_time, fugit, stm32f4xx_hal as hal};

#[macro_use]
mod macros;

// Timer allocation
// TIM2 <-> ADC1
// TIM3 -> display delay
// TIM4 -> sound PWM

pub const CALIBRATION_TIME_MS: u32 = 1000;

pub const TRIGGER_THRESHOLDS: TriggerThresholds = TriggerThresholds {
    low_ratio: 1.0,
    high_ratio: 1.0,
    low_delta: ADC_RANGE / 16u16,
    high_delta: ADC_RANGE / 8u16,
};

// pub const TRIGGER_THRESHOLDS: TriggerThresholds = TriggerThresholds {
//     low_ratio: 1.8,
//     high_ratio: 2.0,
//     low_delta: 0,
//     high_delta: 0,
// };

pub const ADC_RESOLUTION: Resolution = Resolution::Twelve;
pub const ADC_RANGE: u16 = 2u16.pow(match ADC_RESOLUTION {
    Resolution::Six => 6,
    Resolution::Eight => 8,
    Resolution::Ten => 10,
    Resolution::Twelve => 12,
});

pub const SAMPLE_TIME: SampleTime = SampleTime::Cycles_3;
pub const SAMPLE_RATE_HZ: u32 = 100_000_u32;
pub const SYSCLK: u32 = 84_000_000;
pub const HCLK: u32 = 42_000_000;
pub const SPI_FREQ_HZ: u32 = 10_000_000;

pub type DisplaySpiType = ExclusiveDevice<Spi<SPI1>, ErasedPin<Output>, NoDelay>;
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

pub fn _setup_adc(adc: ADC1, adc_pin: Pin<'A', 1, Analog>) -> Adc<ADC1> {
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
        let pin = $crate::adc_pin!($gpio);
        $crate::_setup_adc($dp.ADC1, pin.into_analog())
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
        use embedded_hal_bus;
        use $crate::fugit::RateExtU32;
        use $crate::hal::spi::Spi;

        let mut sclk_pin = hw::display_sclk_pin!($gpio).into_alternate();
        let mut miso_pin = hw::display_miso_pin!($gpio).into_alternate();
        let mut mosi_pin = hw::display_mosi_pin!($gpio).into_alternate();
        let mut dummy_cs_pin = hw::display_dummy_cs_pin!($gpio).into_push_pull_output();
        sclk_pin.set_speed(Speed::VeryHigh);
        miso_pin.set_speed(Speed::VeryHigh);
        mosi_pin.set_speed(Speed::VeryHigh);

        let bus = Spi::new(
            $dp.SPI1,
            (sclk_pin, miso_pin, mosi_pin),
            embedded_hal::spi::MODE_3,
            $crate::SPI_FREQ_HZ.Hz(),
            &$clocks,
        );
        embedded_hal_bus::spi::ExclusiveDevice::new(
            bus,
            dummy_cs_pin.erase(),
            embedded_hal_bus::spi::NoDelay,
        )
        .unwrap()
    }};
}

#[macro_export]
macro_rules! setup_display {
    ($dp:expr, $gpio:expr, $clocks:expr, $delay:expr) => {{
        use $crate::display_interface_spi::SPIInterface;
        use $crate::hal::gpio::{Edge, ErasedPin, Input, Output, Speed};
        let spi = $crate::setup_display_spi!($dp, $gpio, $clocks);
        let mut dc_pin = $crate::display_dc_pin!($gpio).into_push_pull_output();
        let mut rst_pin = $crate::display_rst_pin!($gpio).into_push_pull_output();
        dc_pin.set_speed(Speed::VeryHigh);
        rst_pin.set_speed(Speed::VeryHigh);

        let di = SPIInterface::new(spi, dc_pin.erase());
        mipidsi::Builder::new(mipidsi::models::ST7735s, di)
            .reset_pin(rst_pin.erase())
            .orientation(
                mipidsi::options::Orientation::new().rotate(mipidsi::options::Rotation::Deg180),
            )
            .display_offset(0, 0)
            .display_size(132, 162)
            .init($delay)
    }};
}

#[macro_export]
macro_rules! beeper_type {
    () => {
        use embedded_time::rate::Hertz;
        use $crate::hal::pac::TIM4;
        use $crate::hal::timer::{ChannelBuilder, PwmHz};

        pub struct Beeper {
            pwm: PwmHz<TIM4, ChannelBuilder<TIM4, 0>>,
        }

        impl BeeperExt for Beeper {
            fn enable(&mut self, frequency: f32) {
                use hal::timer::Channel;

                self.pwm.set_period((frequency as u32).Hz());
                self.pwm.enable(Channel::C1);
            }

            fn disable(&mut self) {
                use hal::timer::Channel;
                self.pwm.set_period(10.Hz());
                self.pwm.enable(Channel::C1);
                self.pwm.disable(Channel::C1);
            }

            fn set_duty_percent(&mut self, duty_percent: u8) {
                use hal::timer::Channel;
                self.pwm.set_duty(
                    Channel::C1,
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

        let mut buzzer_pin = $gpio.b.pb6.into_alternate();
        let ch = hal::timer::pwm::Channel1::new(buzzer_pin);
        let mut pwm = $dp.TIM4.pwm_hz(ch, 550.Hz(), $clocks);
        pwm.set_duty(Channel::C1, pwm.get_max_duty() / 2);

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
pin_macro!($ display_dummy_cs_pin, b, pb10);

pin_macro!($ adc_pin, a, pa1);

pin_macro!($ led_pin, c, pc13);

pin_macro!($ measure_button_pin, a, pa2);

pin_macro!($ usb_dm_pin, a, pa11);
pin_macro!($ usb_dp_pin, a, pa12);

pin_macro!($ rotary_dt_pin, c, pc15);
pin_macro!($ rotary_clk_pin, c, pc14);

pin_macro!($ accessory_sense_pin, a, pa3);
pin_macro!($ accessory_idle_signal, b, pb8);

use app_measurements::TriggerThresholds;
use embedded_hal_bus::spi::{ExclusiveDevice, NoDelay};
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
use stm32f4xx_hal::gpio::{ErasedPin, Output};
