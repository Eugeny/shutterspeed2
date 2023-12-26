#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(associated_type_bounds)]
#![feature(iter_array_chunks)]
#![feature(sync_unsafe_cell)]

use core::cell::RefCell;
use core::fmt::Write;
use core::panic::PanicInfo;
use core::sync::atomic::{self, Ordering};

use cortex_m::interrupt::{CriticalSection, Mutex};
use display::Display;
use embedded_alloc::Heap;
use stm32f4xx_hal::pac::SPI1;
use stm32f4xx_hal::spi::Spi;

use crate::ui::draw_panic_screen;
mod display;
mod format;
mod hardware_config;
mod measurement;
mod ui;
mod util;

#[global_allocator]
static HEAP: Heap = Heap::empty();

static PANIC_DISPLAY_REF: Mutex<RefCell<Option<&mut Display<Spi<SPI1>>>>> =
    Mutex::new(RefCell::new(None));

#[rtic::app(device = stm32f4xx_hal::pac, dispatchers = [SPI2, SPI3, SPI4])]
mod app {
    use core::cell::UnsafeCell;
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
    use crate::hardware_config::{HCLK, IPRIO_ADC_TIMER, SAMPLE_RATE_HZ, SAMPLE_TIME, SYSCLK};
    use crate::measurement::{CalibrationState, Measurement, MeasurementResult, RingBuffer};
    use crate::ui::draw_boot_screen;
    use crate::ui::screens::{
        CalibrationScreen, DebugScreen, DebugUiState, MeasurementScreen, ResultsScreen,
        ResultsUiState, Screen, StartScreen,
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
        display: UnsafeCell<Display<Spi<SPI1>>>,
    }

    #[local]
    struct Local {
        adc_dma_buffer: Option<&'static mut u16>,
        timer: CounterHz<TIM2>,
        adc_history: HistoryBuffer<u16, 100>,
        adc_avg_window: HistoryBuffer<u16, 4>,
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

        let gpioa = dp.GPIOA.split();
        let gpiob = dp.GPIOB.split();
        let gpioc = dp.GPIOC.split();
        let mut backlight_pin = gpiob.pb9.into_push_pull_output();
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
            .sysclk(SYSCLK.Hz())
            // .require_pll48clk()
            .hclk(HCLK.MHz())
            .use_hse(25.MHz())
            .pclk1(80.MHz())
            .pclk2(80.MHz())
            .freeze();

        CYCCNTClock::<SYSCLK>::init(&mut cx.core.DCB, cx.core.DWT);

        let systick_token = create_systick_token!();
        Systick::start(cx.core.SYST, SYSCLK, systick_token);

        let mut led_pin = gpioc.pc13.into_push_pull_output();

        let adc_pin = gpioa.pa0.into_analog();
        // Create Handler for adc peripheral (PA0 and PA4 are connected to ADC1)
        // Configure ADC for sequence conversion with interrtups
        let adc_config = AdcConfig::default()
            .dma(Dma::Continuous)
            .scan(Scan::Disabled)
            .clock(Clock::Pclk2_div_6)
            .resolution(Resolution::Eight);

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

        let mut delay = dp.TIM3.delay_us(&clocks);
        let mut display = {
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
                40.MHz(),
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
        // // DEBUG
        // {
        //     ctx.shared.app_mode.lock(|app_mode| {
        //         *app_mode = AppMode::Results;
        //     });
        //     ctx.shared.measurement.lock(|measurement| {
        //         *measurement = Measurement::new_debug_duration(12);
        //     });
        //     return;
        // }

        ctx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        ctx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(1.secs()).await;

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

    #[task(local=[adc_history, adc_avg_window], shared=[adc_value, sample_counter, app_mode, calibration_state, measurement, display], priority=1)]
    async fn display_task(mut cx: display_task::Context) {
        let local = cx.local;

        // Only shared with the panic handler, which never returns
        let display = unsafe { cx.shared.display.lock(|d| &mut *d.get()) };

        draw_boot_screen(display).await;

        let mut start_screen = StartScreen {};
        let mut calibration_screen = CalibrationScreen {};
        let mut measurement_screen = MeasurementScreen {};
        let mut debug_screen = DebugScreen {
            state: DebugUiState {
                adc_value: 0,
                min_adc_value: 0,
                max_adc_value: 0,
                adc_history: HistoryBuffer::new(),
                sample_counter: 0,
            },
        };
        let mut results_screen = ResultsScreen {
            state: ResultsUiState {
                calibration: CalibrationState::Done(0),
                result: MeasurementResult {
                    duration_micros: 0,
                    integrated_duration_micros: 0,
                    sample_buffer: RingBuffer::new(),
                    samples_since_start: 0,
                    samples_since_end: 0,
                },
            },
        };

        let mut mode = AppMode::None;

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
                    AppMode::Calibrating => calibration_screen.draw_init(&mut **display).await,
                    AppMode::Measure => measurement_screen.draw_init(&mut **display).await,
                    AppMode::Debug => debug_screen.draw_init(&mut **display).await,
                    AppMode::Results => results_screen.draw_init(&mut **display).await,
                    AppMode::Start => start_screen.draw_init(&mut **display).await,
                    AppMode::None => (),
                };
            }

            match mode {
                AppMode::Debug => {
                    let adc_value = cx.shared.adc_value.lock(|adc_value| *adc_value);
                    local.adc_history.write(adc_value);
                    local.adc_avg_window.write(adc_value);

                    let min_adc_value = *local.adc_avg_window.iter().min().unwrap_or(&0);
                    let max_adc_value = *local.adc_avg_window.iter().max().unwrap_or(&0);

                    debug_screen.state = DebugUiState {
                        adc_value,
                        min_adc_value,
                        max_adc_value,
                        adc_history: local.adc_history.clone(),
                        // adc_value: avg_adc_value,
                        sample_counter: cx
                            .shared
                            .sample_counter
                            .lock(|sample_counter| sample_counter.0),
                    };
                    debug_screen.draw_frame(&mut **display).await;
                }
                AppMode::Start => start_screen.draw_frame(&mut **display).await,
                AppMode::Calibrating => calibration_screen.draw_frame(&mut **display).await,
                AppMode::Measure => measurement_screen.draw_frame(&mut **display).await,
                AppMode::Results => {
                    let calibration = cx
                        .shared
                        .calibration_state
                        .lock(|calibration_state| calibration_state.clone());
                    let result = cx
                        .shared
                        .measurement
                        .lock(|measurement| measurement.result().cloned())
                        .unwrap();
                    results_screen.state = ResultsUiState {
                        calibration,
                        result,
                    };
                    results_screen.draw_frame(&mut **display).await;
                }
                AppMode::None => {}
            }

            Systick::delay_until(now + 250.millis()).await;
        }
    }

    #[idle(shared=[display])]
    fn idle(mut cx: idle::Context) -> ! {
        cx.shared.display.lock(|display| {
            cortex_m::interrupt::free(|cs| {
                *crate::PANIC_DISPLAY_REF.borrow(cs).borrow_mut() =
                    Some(unsafe { &mut *display.get() });
            });
        });

        loop {
            rtic::export::wfi()
        }
    }
}

#[inline(never)]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // We're dying, go all out just this once
    let cs = unsafe { CriticalSection::new() };
    let display = PANIC_DISPLAY_REF.borrow(&cs).borrow_mut().take().unwrap();

    unsafe {
        cortex_m::interrupt::enable();
    }

    let mut message = heapless::String::<256>::default();

    if write!(message, "{info}").is_err() {
        let _ = write!(message, "Could not format panic message");
    }

    draw_panic_screen(&mut **display, message.as_ref());

    cortex_m::interrupt::disable();

    loop {
        // add some side effect to prevent this from turning into a UDF instruction
        // see rust-lang/rust#28728 for details
        atomic::compiler_fence(Ordering::SeqCst);
    }
}
