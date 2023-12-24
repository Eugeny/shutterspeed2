#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(associated_type_bounds)]
#![feature(iter_array_chunks)]

use panic_halt as _;
mod display;
mod format;
mod hardware_config;
mod measurement;
mod ui;
mod util;

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [SPI2, SPI3])]
mod app {
    use core::num::Wrapping;

    use cortex_m_microclock::CYCCNTClock;
    use hal::adc::config::{AdcConfig, Clock, Dma, Resolution, Scan, Sequence};
    use hal::adc::Adc;
    use hal::dma::config::DmaConfig;
    use hal::dma::{PeripheralToMemory, Stream0, StreamsTuple, Transfer};
    use hal::gpio::{Edge, ErasedPin, Input, Speed};
    use hal::pac::{self, Interrupt, ADC1, DMA2, SPI1, TIM2};
    use hal::prelude::*;
    use hal::spi::Spi;
    use hal::timer::{CounterHz, Event, Flag};
    use heapless::HistoryBuffer;
    use rtic_monotonics::systick::Systick;
    use rtic_monotonics::{create_systick_token, Monotonic};
    use stm32f4xx_hal as hal;

    use crate::display::Display;
    use crate::hardware_config::{
        self as hw_cfg, HCLK, IPRIO_ADC_TIMER, SAMPLE_RATE_HZ, SAMPLE_TIME, SYSCLK,
    };
    use crate::measurement::{CalibrationState, Measurement};
    use crate::ui::{
        draw_debug_ui, draw_measuring_ui, draw_results_ui, draw_start_ui, init_calibrating_ui,
        init_debug_ui, init_measuring_ui, init_results_ui, init_start_ui, DebugUiState,
        ResultsUiState,
    };
    use crate::util::CycleCounterClock;

    type DMATransfer = Transfer<Stream0<DMA2>, 0, Adc<ADC1>, PeripheralToMemory, &'static mut u16>;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AppMode {
        None,
        Start,
        Calibrating,
        Measure,
        Results,
        Debug,
    }

    #[shared]
    struct Shared {
        transfer: DMATransfer,
        adc_value: u16,
        sample_counter: Wrapping<u32>,
        app_mode: AppMode,
        calibration_state: CalibrationState,
        measurement: Measurement<CycleCounterClock<SYSCLK>>,
    }

    #[local]
    struct Local {
        adc_dma_buffer: Option<&'static mut u16>,
        timer: CounterHz<TIM2>,
        display: Display<Spi<SPI1>>,
        adc_history: HistoryBuffer<u16, 100>,
        adc_avg_window: HistoryBuffer<u16, 4>,
        mode_button_pin: ErasedPin<Input>,
        measure_button_pin: ErasedPin<Input>,
    }

    #[init(local = [first_buffer: u16 = 0, _adc_dma_buffer: u16 = 0])]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        let mut dp: pac::Peripherals = cx.device;

        // Workaround 1 enable prefetch
        // {
        //     dp.FLASH
        //         .acr
        //         .write(|w| w.prften().enabled().icen().enabled().dcen().enabled());
        // }

        // // Workaround 2 AN4073 4.1 reduce ADC crosstalk
        // {
        //     dp.PWR.cr.write(|w| w.adcdc1().set_bit());
        // }

        // Workaround 3 AN4073 4.1 reduce ADC crosstalk
        {
            dp.SYSCFG.pmc.write(|x| x.adc1dc2().set_bit())
        }

        let mut syscfg = dp.SYSCFG.constrain();

        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .sysclk(SYSCLK.Hz())
            // .require_pll48clk()
            .hclk(HCLK.MHz())
            .use_hse(25.MHz())
            .pclk1(40.MHz())
            .pclk2(40.MHz())
            .freeze();

        CYCCNTClock::<SYSCLK>::init(&mut cx.core.DCB, cx.core.DWT);

        let systick_token = create_systick_token!();
        Systick::start(cx.core.SYST, SYSCLK, systick_token);

        let gpioa = dp.GPIOA.split();
        let gpiob = dp.GPIOB.split();
        let gpioc = dp.GPIOC.split();

        let mut delay = dp.TIM3.delay_us(&clocks);

        let mut led_pin = gpioc.pc13.into_push_pull_output();

        let adc_pin = gpioa.pa0.into_analog();
        // Create Handler for adc peripheral (PA0 and PA4 are connected to ADC1)
        // Configure ADC for sequence conversion with interrtups
        let adc_config = AdcConfig::default()
            .dma(Dma::Continuous)
            .scan(Scan::Disabled)
            .clock(Clock::Pclk2_div_6)
            .resolution(Resolution::Twelve);

        let mut adc = Adc::adc1(dp.ADC1, true, adc_config);
        adc.configure_channel(&adc_pin, Sequence::One, SAMPLE_TIME);

        // DMA Configuration
        let dma = StreamsTuple::new(dp.DMA2);
        let dma_config = DmaConfig::default()
            .transfer_complete_interrupt(true)
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
        timer.start(SAMPLE_RATE_HZ.Hz()).unwrap();

        unsafe {
            cx.core.NVIC.set_priority(Interrupt::TIM2, IPRIO_ADC_TIMER);
        }

        //----

        let mut pwm = dp
            .TIM4
            .pwm_hz(hal::timer::Channel4::new(gpiob.pb9), 100.Hz(), &clocks);
        pwm.enable(hal::timer::Channel::C4);
        pwm.set_duty(hal::timer::Channel::C4, 0);

        let display = {
            let mut dc_pin = gpioa.pa8.into_push_pull_output();
            let mut rst_pin = gpioa.pa11.into_push_pull_output();
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
                5.MHz(),
                &clocks,
            );
            let mut display = Display::new(spi, dc_pin.erase(), rst_pin.erase(), &mut delay);
            display.clear();
            display
        };

        pwm.set_duty(
            hal::timer::Channel::C4,
            (pwm.get_max_duty() as f32 * hw_cfg::DISPLAY_BRIGHTNESS) as u16,
        );

        let mut mode_button_pin = gpioa.pa1.into_pull_down_input();
        mode_button_pin.make_interrupt_source(&mut syscfg);
        mode_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        mode_button_pin.enable_interrupt(&mut dp.EXTI);

        let mut measure_button_pin = gpioa.pa2.into_pull_down_input();
        measure_button_pin.make_interrupt_source(&mut syscfg);
        measure_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        measure_button_pin.enable_interrupt(&mut dp.EXTI);

        display_task::spawn().unwrap();
        led_pin.set_low();

        (
            Shared {
                transfer,
                adc_value: 0,
                sample_counter: Wrapping(0),
                app_mode: AppMode::Start,
                calibration_state: CalibrationState::Done(0),
                measurement: Measurement::new(0),
            },
            Local {
                adc_dma_buffer: Some(cx.local._adc_dma_buffer),
                timer,
                display,
                adc_history: HistoryBuffer::new(),
                adc_avg_window: HistoryBuffer::new(),
                mode_button_pin: mode_button_pin.erase(),
                measure_button_pin: measure_button_pin.erase(),
            },
        )
    }

    #[task(binds = TIM2, shared = [transfer], local = [timer], priority = 3)]
    fn adcstart(mut cx: adcstart::Context) {
        cx.shared.transfer.lock(|transfer| {
            transfer.start(|adc| {
                adc.start_conversion();
            });
        });
        cx.local.timer.clear_flags(Flag::Update);
    }

    #[task(binds = EXTI1, shared = [app_mode], local=[mode_button_pin], priority = 4)]
    fn mode_button_press(mut ctx: mode_button_press::Context) {
        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = match *app_mode {
                AppMode::None | AppMode::Results | AppMode::Start | AppMode::Measure => {
                    AppMode::Debug
                }
                AppMode::Debug => AppMode::Start,
                x => x,
            }
        });
        ctx.local.mode_button_pin.clear_interrupt_pending_bit();
    }

    #[task(binds = EXTI2, shared = [app_mode], local=[measure_button_pin], priority = 4)]
    fn measure_button_press(mut ctx: measure_button_press::Context) {
        ctx.shared.app_mode.lock(|app_mode| {
            if *app_mode == AppMode::Measure {
                *app_mode = AppMode::Start;
            } else {
                let _ = measure_task::spawn();
            }
        });

        ctx.local.measure_button_pin.clear_interrupt_pending_bit();
    }

    #[task(binds = DMA2_STREAM0, shared = [transfer, adc_value, sample_counter, calibration_state, measurement], local = [adc_dma_buffer], priority = 3)]
    fn dma(ctx: dma::Context) {
        let mut shared = ctx.shared;
        let local = ctx.local;

        let last_adc_dma_buffer = shared.transfer.lock(|transfer| {
            let (last_adc_dma_buffer, _) = transfer
                .next_transfer(local.adc_dma_buffer.take().unwrap())
                .unwrap();
            last_adc_dma_buffer
        });

        let value = *last_adc_dma_buffer;
        // Return adc_dma_buffer to resources pool for next transfer
        *local.adc_dma_buffer = Some(last_adc_dma_buffer);

        (
            shared.adc_value,
            shared.calibration_state,
            shared.measurement,
            shared.sample_counter,
        )
            .lock(
                |adc_value, calibration_state, measurement, sample_counter| {
                    if let CalibrationState::InProgress(ref mut calibration) = calibration_state {
                        calibration.add(value)
                    } else {
                        measurement.step(value);
                    }
                    *adc_value = value;
                    *sample_counter += Wrapping(1);
                },
            );
    }

    #[task(shared=[app_mode, adc_value, calibration_state, measurement], priority=2)]
    async fn measure_task(mut ctx: measure_task::Context) {
        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        ctx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(1.secs().into()).await;

        let calibration_value = ctx.shared.calibration_state.lock(|state| state.finish());
        ctx.shared.measurement.lock(|measurement| {
            *measurement = Measurement::new(calibration_value);
        });

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Measure;
        });

        loop {
            ctx.shared.app_mode.lock(|app_mode| {
                if *app_mode != AppMode::Measure {
                    // Cancelled
                    return;
                }
            });

            if ctx
                .shared
                .measurement
                .lock(|measurement| measurement.is_done())
            {
                break;
            }

            Systick::delay(100.millis().into()).await;
        }

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Results;
        });
    }

    #[task(local=[display, adc_history, adc_avg_window], shared=[adc_value, sample_counter, app_mode, calibration_state, measurement], priority=1)]
    async fn display_task(mut ctx: display_task::Context) {
        let local = ctx.local;
        init_calibrating_ui(local.display);

        let mut mode = AppMode::None;

        loop {
            let now = Systick::now();
            ctx.shared.app_mode.lock(|app_mode| {
                if *app_mode != mode {
                    mode = *app_mode;
                    match mode {
                        AppMode::Calibrating => init_calibrating_ui(local.display),
                        AppMode::Measure => init_measuring_ui(local.display),
                        AppMode::Debug => init_debug_ui(local.display),
                        AppMode::Results => init_results_ui(local.display),
                        AppMode::Start => init_start_ui(local.display),
                        AppMode::None => {}
                    };
                }
            });

            match mode {
                AppMode::Debug => {
                    let adc_value = ctx.shared.adc_value.lock(|adc_value| *adc_value);
                    local.adc_history.write(adc_value);
                    local.adc_avg_window.write(adc_value);

                    // let avg_adc_value = local.adc_avg_window.iter().sum::<u16>()
                    //     / local.adc_avg_window.len() as u16;

                    let min_adc_value = *local.adc_avg_window.iter().min().unwrap_or(&0);
                    let max_adc_value = *local.adc_avg_window.iter().max().unwrap_or(&0);

                    draw_debug_ui(
                        local.display,
                        &mut DebugUiState {
                            adc_value,
                            min_adc_value,
                            max_adc_value,
                            adc_history: local.adc_history,
                            // adc_value: avg_adc_value,
                            sample_counter: ctx
                                .shared
                                .sample_counter
                                .lock(|sample_counter| sample_counter.0),
                        },
                    );
                }
                AppMode::Start => draw_start_ui(local.display),
                AppMode::Calibrating => {}
                AppMode::Measure => draw_measuring_ui(local.display),
                AppMode::Results => {
                    let calibration = ctx
                        .shared
                        .calibration_state
                        .lock(|calibration_state| calibration_state.clone());
                    let result = ctx
                        .shared
                        .measurement
                        .lock(|measurement| measurement.result().cloned())
                        .unwrap();
                    let result_samples = ctx
                        .shared
                        .measurement
                        .lock(|measurement| measurement.result_samples());
                    draw_results_ui(
                        local.display,
                        &ResultsUiState {
                            calibration,
                            result,
                            result_samples,
                        },
                    )
                }
                AppMode::None => {}
            }

            Systick::delay_until(now + 250.millis()).await;
        }
    }

    #[idle]
    fn idle(_cx: idle::Context) -> ! {
        loop {
            rtic::export::wfi()
        }
    }
}
