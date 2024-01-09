#![no_std]

pub use stm32f4xx_hal as hal;

#[macro_use]
mod macros;

// Timer allocation
// TIM2 <-> ADC1
// TIM3 -> display delay

pub const CALIBRATION_TIME_MS: u32 = 1000;
pub const TRIGGER_THRESHOLD_LOW: f32 = 1.3;
pub const TRIGGER_THRESHOLD_HIGH: f32 = 1.5;
pub const ADC_RESOLUTION: Resolution = Resolution::Eight;

pub const SAMPLE_TIME: SampleTime = SampleTime::Cycles_3;
pub const SAMPLE_RATE_HZ: u32 = 50_000_u32;
pub const SYSCLK: u32 = 84_000_000;
pub const HCLK: u32 = 42_000_000;
pub const SPI_FREQ_HZ: u32 = 35_000_000;

pub type DisplaySpiType = Spi<SPI1>;
pub type DmaTransfer = Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut u16>;
pub type AdcTimerType = CounterHz<TIM2>;

#[macro_export]
macro_rules! setup_clocks {
    ($dp:expr) => {{
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

#[macro_export]
macro_rules! setup_adc_timer {
    ($core:expr, $dp:expr, $clocks:expr) => {{
        use hal::timer::Event;

        let mut timer = $dp.TIM2.counter_hz($clocks);
        timer.listen(Event::Update);
        timer.start($crate::SAMPLE_RATE_HZ.Hz()).unwrap();

        const IPRIO_ADC_TIMER: u8 = 5;

        unsafe {
            $core.NVIC.set_priority(Interrupt::TIM2, IPRIO_ADC_TIMER);
        }

        timer
    }};
}

#[macro_export]
macro_rules! setup_adc {
    ($dp:expr, $gpio:expr) => {{
        use hal::adc::config::{AdcConfig, Clock, Dma, Resolution, SampleTime, Scan, Sequence};
        use hal::adc::Adc;

        let adc_pin = $gpio.a.pa0.into_analog();
        // Create Handler for adc peripheral (PA0 and PA4 are connected to ADC1)
        // Configure ADC for sequence conversion with interrtups
        let adc_config = AdcConfig::default()
            .dma(Dma::Continuous)
            .scan(Scan::Disabled)
            .clock(Clock::Pclk2_div_6)
            .resolution($crate::ADC_RESOLUTION);

        let mut adc = Adc::adc1($dp.ADC1, true, adc_config);
        adc.configure_channel(&adc_pin, Sequence::One, $crate::SAMPLE_TIME);

        adc
    }};
}

#[macro_export]
macro_rules! setup_adc_dma_transfer {
    ($dp:expr, $adc:expr, $buffer:expr) => {{
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
        use hal::spi::Spi;

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

pub struct AllGpio {
    pub a: hal::gpio::gpioa::Parts,
    pub b: hal::gpio::gpiob::Parts,
    pub c: hal::gpio::gpioc::Parts,
}

pin_macro!($ display_dc_pin, a, pa8);
pin_macro!($ display_rst_pin, a, pa10);
pin_macro!($ display_sclk_pin, a, pa5);
pin_macro!($ display_miso_pin, a, pa6);
pin_macro!($ display_mosi_pin, a, pa7);
pin_macro!($ display_backlight_pin, b, pb9);

pin_macro!($ adc_pin, a, pa0);

pin_macro!($ led_pin, c, pc13);

pin_macro!($ mode_button_pin, a, pa1);
pin_macro!($ measure_button_pin, a, pa2);

pin_macro!($ usb_dm_pin, a, pa11);
pin_macro!($ usb_dp_pin, a, pa12);

#[macro_export]
macro_rules! rtic_app {
    ({ $($module:item)* }) => {
        #[rtic::app(device = hal::pac, dispatchers = [SPI2, SPI3, SPI4])]
        mod app {
            $($module)*
        }
    };
}

use hal::adc::config::{Resolution, SampleTime};
use hal::adc::Adc;
use hal::dma::{PeripheralToMemory, Stream0, Transfer};
use hal::pac::{ADC1, DMA2, SPI1, TIM2};
use hal::spi::Spi;
use hal::timer::CounterHz;
