#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(associated_type_bounds)]
#![feature(iter_array_chunks)]
#![feature(sync_unsafe_cell)]

use embedded_alloc::Heap;
mod display;
mod format;
mod hardware_config;
mod measurement;
mod panic;
mod ui;
mod util;

#[global_allocator]
static HEAP: Heap = Heap::empty();

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [SPI2, SPI3, SPI4])]
mod app {
    use core::cell::UnsafeCell;
    use core::num::Wrapping;
    use core::panic;

    use cortex_m_microclock::CYCCNTClock;
    use enum_dispatch::enum_dispatch;
    use hal::adc::config::{AdcConfig, Clock, Dma, Scan, Sequence};
    use hal::adc::Adc;
    use hal::dma::config::DmaConfig;
    use hal::dma::{PeripheralToMemory, Stream0, StreamsTuple, Transfer};
    use hal::gpio::{Edge, ErasedPin, Input, Speed};
    use hal::pac::{self, Interrupt, ADC1, DMA2, TIM2};
    use hal::prelude::*;
    use hal::spi::Spi;
    use hal::timer::{CounterHz, Event, Flag};
    use rtic_monotonics::systick::Systick;
    use rtic_monotonics::{create_systick_token, Monotonic};
    use stm32f4xx_hal as hal;

    use crate::display::{AppDrawTarget, Display};
    use crate::hardware_config::{self as hw, AllGpio, DisplayType};
    use crate::measurement::{CalibrationState, Measurement};
    use crate::panic::set_panic_display_ref;
    use crate::ui::draw_boot_screen;
    use crate::ui::screens::{
        CalibrationScreen, DebugScreen, MeasurementScreen, ResultsScreen, Screen, StartScreen,
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
        measurement: Measurement<CycleCounterClock<{ hw::SYSCLK }>>,
        display: UnsafeCell<DisplayType>,
    }

    #[local]
    struct Local {
        adc_dma_buffer: Option<&'static mut u16>,
        timer: CounterHz<TIM2>,
        mode_button_pin: ErasedPin<Input>,
        measure_button_pin: ErasedPin<Input>,
    }

    #[init(local = [first_buffer: u16 = 0, _adc_dma_buffer: u16 = 0])]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        {
            use core::mem::MaybeUninit;
            const HEAP_SIZE: usize = 1024;
            static mut HEAP_MEM: [MaybeUninit<u8>; HEAP_SIZE] = [MaybeUninit::uninit(); HEAP_SIZE];
            unsafe { crate::HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE) }
        }

        let mut dp: pac::Peripherals = cx.device;

        let gpio = AllGpio {
            a: dp.GPIOA.split(),
            b: dp.GPIOB.split(),
            c: dp.GPIOC.split(),
        };

        let mut backlight_pin = hw::display_backlight_pin!(gpio).into_push_pull_output();
        backlight_pin.set_low();

        // Workaround 1 enable prefetch
        // {
        //     dp.FLASH
        //         .acr
        //         .write(|w| w.prften().enabled().icen().enabled().dcen().enabled());
        // }

        // Workaround 2 AN4073 4.1 reduce ADC crosstalk
        {
            dp.PWR.cr.write(|w| w.adcdc1().set_bit());
        }

        // // Workaround 3 AN4073 4.1 reduce ADC crosstalk
        // {
        //     dp.SYSCFG.pmc.write(|x| x.adc1dc2().set_bit())
        // }

        let mut syscfg = dp.SYSCFG.constrain();

        let rcc = dp.RCC.constrain();
        let clocks = rcc
            .cfgr
            .sysclk(hw::SYSCLK.Hz())
            // .require_pll48clk()
            .hclk(hw::HCLK.MHz())
            .use_hse(25.MHz())
            .pclk1(80.MHz())
            .pclk2(80.MHz())
            .freeze();

        CYCCNTClock::<{ hw::SYSCLK }>::init(&mut cx.core.DCB, cx.core.DWT);

        let systick_token = create_systick_token!();
        Systick::start(cx.core.SYST, hw::SYSCLK, systick_token);

        let mut led_pin = hw::led_pin!(gpio).into_push_pull_output();

        let adc_pin = hw::adc_pin!(gpio).into_analog();
        // Create Handler for adc peripheral (PA0 and PA4 are connected to ADC1)
        // Configure ADC for sequence conversion with interrtups
        let adc_config = AdcConfig::default()
            .dma(Dma::Continuous)
            .scan(Scan::Disabled)
            .clock(Clock::Pclk2_div_6)
            .resolution(hw::ADC_RESOLUTION);

        let mut adc = Adc::adc1(dp.ADC1, true, adc_config);
        adc.configure_channel(&adc_pin, Sequence::One, hw::SAMPLE_TIME);

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
        timer.start(hw::SAMPLE_RATE_HZ.Hz()).unwrap();

        unsafe {
            cx.core
                .NVIC
                .set_priority(Interrupt::TIM2, hw::IPRIO_ADC_TIMER);
        }

        //----

        let mut delay = dp.TIM3.delay_us(&clocks);
        let mut display = {
            let mut dc_pin = hw::display_dc_pin!(gpio).into_push_pull_output();
            let mut rst_pin = hw::display_rst_pin!(gpio).into_push_pull_output();
            let mut sclk_pin = hw::display_sclk_pin!(gpio).into_alternate();
            let mut miso_pin = hw::display_miso_pin!(gpio).into_alternate();
            let mut mosi_pin = hw::display_mosi_pin!(gpio).into_alternate();

            dc_pin.set_speed(Speed::VeryHigh);
            rst_pin.set_speed(Speed::VeryHigh);
            sclk_pin.set_speed(Speed::VeryHigh);
            miso_pin.set_speed(Speed::VeryHigh);
            mosi_pin.set_speed(Speed::VeryHigh);
            let spi = Spi::new(
                dp.SPI1,
                (sclk_pin, miso_pin, mosi_pin),
                embedded_hal::spi::MODE_3,
                hw::SPI_FREQ_HZ.Hz(),
                &clocks,
            );
            let mut display = Display::new(
                spi,
                dc_pin.erase(),
                rst_pin.erase(),
                backlight_pin.erase(),
                &mut delay,
            );
            display.clear();
            display
        };

        display.backlight_on();

        let mut mode_button_pin = hw::mode_button_pin!(gpio).into_pull_down_input();
        mode_button_pin.make_interrupt_source(&mut syscfg);
        mode_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        mode_button_pin.enable_interrupt(&mut dp.EXTI);

        let mut measure_button_pin = hw::measure_button_pin!(gpio).into_pull_down_input();
        measure_button_pin.make_interrupt_source(&mut syscfg);
        measure_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        measure_button_pin.enable_interrupt(&mut dp.EXTI);

        display_task::spawn().unwrap();
        led_pin.set_low();

        let display = UnsafeCell::new(display);

        (
            Shared {
                transfer,
                adc_value: 0,
                sample_counter: Wrapping(0),
                app_mode: AppMode::Start,
                calibration_state: CalibrationState::Done(0),
                measurement: Measurement::new(0),
                display,
            },
            Local {
                adc_dma_buffer: Some(cx.local._adc_dma_buffer),
                timer,
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
    fn mode_button_press(mut cx: mode_button_press::Context) {
        cx.shared.app_mode.lock(|app_mode| match app_mode {
            AppMode::None | AppMode::Results | AppMode::Start | AppMode::Measure => {
                debug_task::spawn().unwrap();
            }
            AppMode::Debug => {
                *app_mode = AppMode::Start;
                debug_task::spawn().unwrap();
            }
            _ => (),
        });
        cx.local.mode_button_pin.clear_interrupt_pending_bit();
    }

    #[task(binds = EXTI2, shared = [app_mode], local=[measure_button_pin], priority = 4)]
    fn measure_button_press(mut cx: measure_button_press::Context) {
        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Start;
        });
        let _ = measure_task::spawn();
        cx.local.measure_button_pin.clear_interrupt_pending_bit();
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
        // // DEBUG
        // {
        //     ctx.shared.app_mode.lock(|app_mode| {
        //         *app_mode = AppMode::Results;
        //     });
        //     ctx.shared.measurement.lock(|measurement| {
        //         *measurement = Measurement::new_debug_duration(7);
        //     });
        //     return;
        // }

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        ctx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(hw::CALIBRATION_TIME_MS.millis()).await;

        let calibration_value = ctx.shared.calibration_state.lock(|state| state.finish());
        ctx.shared.measurement.lock(|measurement| {
            *measurement = Measurement::new(calibration_value);
        });

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Measure;
        });

        loop {
            if ctx.shared.app_mode.lock(|app_mode| *app_mode) != AppMode::Measure {
                // Cancelled
                return;
            }

            if ctx
                .shared
                .measurement
                .lock(|measurement| measurement.is_done())
            {
                break;
            }

            Systick::delay(100.millis()).await;
        }

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Results;
        });
    }

    #[task(shared=[app_mode, calibration_state], priority=2)]
    async fn debug_task(mut ctx: debug_task::Context) {
        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        ctx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(hw::CALIBRATION_TIME_MS.millis()).await;

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Debug;
        });
    }

    #[enum_dispatch(Screen)]
    #[allow(clippy::large_enum_variant, clippy::enum_variant_names)]
    enum Screens {
        StartScreen,
        CalibrationScreen,
        MeasurementScreen,
        DebugScreen,
        ResultsScreen,
    }

    #[task(shared=[adc_value, sample_counter, app_mode, calibration_state, measurement, display], priority=1)]
    async fn display_task(mut cx: display_task::Context) {
        // Only shared with the panic handler, which never returns
        let display = unsafe { cx.shared.display.lock(|d| &mut *d.get()) };

        draw_boot_screen(display).await;

        let mut mode = AppMode::None;
        let mut screen: Screens = StartScreen {}.into();

        loop {
            let now = Systick::now();

            if let Some(changed_mode) = cx.shared.app_mode.lock(|app_mode| {
                if *app_mode != mode {
                    mode = *app_mode;
                    return Some(mode);
                }
                None
            }) {
                match changed_mode {
                    AppMode::Start => {
                        screen = StartScreen {}.into();
                    }
                    AppMode::Calibrating => {
                        screen = CalibrationScreen {}.into();
                    }
                    AppMode::Measure => {
                        screen = MeasurementScreen {}.into();
                    }
                    AppMode::Debug => {
                        screen = DebugScreen::new(
                            cx.shared.calibration_state.lock(core::mem::take).finish(),
                        )
                        .into();
                    }
                    AppMode::Results => {
                        let calibration = cx.shared.calibration_state.lock(core::mem::take);
                        let result = cx
                            .shared
                            .measurement
                            .lock(core::mem::take)
                            .take_result()
                            .unwrap();
                        screen = ResultsScreen {
                            calibration,
                            result,
                        }
                        .into();
                    }
                    AppMode::None => (),
                };
                screen.draw_init(&mut **display).await;
            }

            match screen {
                Screens::DebugScreen(ref mut screen) => {
                    let adc_value = cx.shared.adc_value.lock(|adc_value| *adc_value);
                    screen.step(adc_value);
                }
                _ => (),
            }

            screen.draw_frame(&mut **display).await;

            let deadline = if matches!(screen, Screens::DebugScreen(_)) {
                Systick::now() + 5.millis()
            } else {
                now + 250.millis()
            };
            Systick::delay_until(deadline).await;
        }
    }

    #[idle(shared=[display])]
    fn idle(mut cx: idle::Context) -> ! {
        cx.shared.display.lock(|display| {
            set_panic_display_ref(display);
        });

        loop {
            rtic::export::wfi()
        }
    }

    #[task(binds=BusFault)]
    fn bus_fault(_cx: bus_fault::Context) {
        panic!("BusFault");
    }

    #[task(binds=UsageFault)]
    fn usage_fault(_cx: usage_fault::Context) {
        panic!("UsageFault");
    }
}
