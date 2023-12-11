#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

use panic_halt as _;
mod display;
mod ui;
mod util;
use display::Display;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [])]
mod app {
    use hal::spi::Spi;
    use rtic_monotonics::create_systick_token;
    use rtic_monotonics::systick::Systick;
    use stm32f4xx_hal as hal;
    use stm32f4xx_hal::adc::config::{
        AdcConfig, Clock, Dma, Resolution, SampleTime, Scan, Sequence,
    };
    use stm32f4xx_hal::adc::Adc;
    use stm32f4xx_hal::dma::config::DmaConfig;
    use stm32f4xx_hal::dma::{PeripheralToMemory, Stream0, StreamsTuple, Transfer};
    use stm32f4xx_hal::gpio::Speed;
    use stm32f4xx_hal::pac::{self, ADC1, DMA2, SPI1, TIM2};
    use stm32f4xx_hal::prelude::*;
    use stm32f4xx_hal::timer::{CounterHz, Event, Flag};

    use crate::ui::{draw_ui, UiState};

    const DISPLAY_BRIGHTNESS: f32 = 0.1;
    const SAMPLE_TIME: SampleTime = SampleTime::Cycles_112;

    type DMATransfer = Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut u16>;

    #[shared]
    struct Shared {
        transfer: DMATransfer,
        adc_value: u16,
    }

    #[local]
    struct Local {
        buffer: Option<&'static mut u16>,
        timer: CounterHz<TIM2>,
        display: crate::Display<Spi<SPI1>>,
    }

    #[init(local = [first_buffer: u16 = 0, second_buffer: u16 = 0])]
    fn init(cx: init::Context) -> (Shared, Local) {
        let dp: pac::Peripherals = cx.device;

        // // Clock Configuration
        // let rcc = dp.RCC.constrain();
        // let clocks = rcc
        //     .cfgr
        //     .use_hse(8.MHz())
        //     .sysclk(84.MHz())
        //     .hclk(84.MHz())
        //     .require_pll48clk()
        //     .pclk2(21.MHz())
        //     .freeze();
        dp.RCC.apb2enr.write(|w| w.syscfgen().enabled());
        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .hclk(84.MHz())
            // .use_hse(25.MHz())
            .sysclk(48.MHz())
            .freeze();

        let gpioa = dp.GPIOA.split();
        let gpiob = dp.GPIOB.split();
        let gpioc = dp.GPIOC.split();

        let mut delay = dp.TIM1.delay_us(&clocks);

        let _led_pin = gpioc.pc13.into_push_pull_output();

        let systick_token = create_systick_token!();
        Systick::start(cx.core.SYST, 12_000_000, systick_token);

        let mic1 = gpioa.pa0.into_analog();
        // Create Handler for adc peripheral (PA0 and PA4 are connected to ADC1)
        // Configure ADC for sequence conversion with interrtups
        let adc_config = AdcConfig::default()
            .dma(Dma::Continuous)
            .scan(Scan::Enabled)
            .resolution(Resolution::Twelve)
            .clock(Clock::Pclk2_div_8);

        let mut adc = Adc::adc1(dp.ADC1, true, adc_config);
        adc.configure_channel(&mic1, Sequence::One, SampleTime::Cycles_480);

        // DMA Configuration
        let dma = StreamsTuple::new(dp.DMA2);
        let dma_config = DmaConfig::default()
            .transfer_complete_interrupt(true)
            .memory_increment(true)
            .double_buffer(false);

        let transfer = Transfer::init_peripheral_to_memory(
            dma.0,
            adc,
            cx.local.first_buffer,
            None,
            dma_config,
        );

        let mut timer = dp.TIM2.counter_hz(&clocks);
        timer.listen(Event::Update);
        timer.start(1000.Hz()).unwrap();

        //----

        let mut pwm = dp
            .TIM4
            .pwm_hz(hal::timer::Channel4::new(gpiob.pb9), 100.Hz(), &clocks);
        pwm.enable(hal::timer::Channel::C4);
        pwm.set_duty(hal::timer::Channel::C4, 0);

        let display = {
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
            let mut display = super::Display::new(spi, dc_pin.erase(), rst_pin.erase(), &mut delay);
            display.clear();
            display
        };

        pwm.set_duty(
            hal::timer::Channel::C4,
            (pwm.get_max_duty() as f32 * DISPLAY_BRIGHTNESS) as u16,
        );

        display_task::spawn().unwrap();

        (
            Shared {
                transfer,
                adc_value: 0,
            },
            Local {
                buffer: Some(cx.local.second_buffer),
                timer,
                display,
            },
        )
    }

    #[task(binds = TIM2, shared = [transfer], local = [timer])]
    fn adcstart(mut cx: adcstart::Context) {
        cx.shared.transfer.lock(|transfer| {
            transfer.start(|adc| {
                adc.start_conversion();
            });
        });
        cx.local.timer.clear_flags(Flag::Update);
    }

    #[task(binds = DMA2_STREAM0, shared = [transfer, adc_value], local = [buffer])]
    fn dma(mut ctx: dma::Context) {
        // Destructure dma::Context to make only the shared resources mutable
        //let dma::Context { mut shared, local } = cx;

        // Also Equivalent to
        let mut shared = ctx.shared;
        let local = ctx.local;

        let buffer = shared.transfer.lock(|transfer| {
            let (buffer, _) = transfer
                .next_transfer(local.buffer.take().unwrap())
                .unwrap();
            buffer
        });

        let mic1 = *buffer;

        shared.adc_value.lock(|adc_value| {
            *adc_value = mic1;
        });

        // Return buffer to resources pool for next transfer
        *local.buffer = Some(buffer);
    }

    #[task(local=[display], shared=[adc_value])]
    async fn display_task(mut ctx: display_task::Context) {
        let local = ctx.local;
        loop {
            let adc_value = ctx.shared.adc_value.lock(|adc_value| *adc_value);

            draw_ui(local.display, &UiState { adc_value });
            Systick::delay(100.millis()).await;
        }
    }
}
