#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(associated_type_bounds)]
#![feature(iter_array_chunks)]
#![feature(sync_unsafe_cell)]

mod display;
mod panic;
mod sound;

// HWCONFIG
#[rtic::app(device = hal::pac, dispatchers = [SPI2, SPI3, SPI4])]
mod app {
    use core::cell::UnsafeCell;
    use core::num::Wrapping;
    use core::panic;

    use app_measurements::{CalibrationState, CycleCounterClock, Measurement, RingBuffer};
    use app_ui::{
        BootScreen, CalibrationScreen, DebugScreen, MeasurementScreen, MenuScreen, ResultsScreen,
        Screen, Screens, StartScreen, UpdateScreen,
    };
    use config::{self as hw, hal, AllGpio};
    #[cfg(usb)]
    use cortex_m::peripheral::NVIC;
    use cortex_m_microclock::CYCCNTClock;
    use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
    use hal::adc::config::Resolution;
    use hal::gpio::{Edge, ErasedPin, Input, Output, Speed};
    #[cfg(usb)]
    use hal::otg_fs::{UsbBus, UsbBusType, USB};
    use hal::pac::{self, Interrupt};
    use hal::prelude::*;
    use hal::timer::Flag;
    use mipidsi::Error as MipidsiError;
    #[cfg(usb)]
    use ouroboros::self_referencing;
    use rotary_encoder_embedded::standard::StandardMode;
    use rotary_encoder_embedded::{Direction as EncoderDirection, RotaryEncoder};
    use rtic_monotonics::systick::Systick;
    use rtic_monotonics::{create_systick_token, Monotonic};
    use rtic_sync::channel::{Receiver, Sender};
    use rtic_sync::make_channel;
    use stm32f4xx_hal::dma::traits::Direction;
    #[cfg(usb)]
    use usb_device::class_prelude::UsbBusAllocator;
    #[cfg(usb)]
    use usb_device::device::{StringDescriptors, UsbDevice, UsbDeviceBuilder, UsbVidPid};
    #[cfg(usb)]
    use usbd_serial::SerialPort;

    use crate::display::Display;
    use crate::panic::set_panic_display_ref;
    use crate::sound::{BeeperExt, Chirp};

    pub type DisplayType = Display<config::DisplaySpiType>;

    config::beeper_type!();

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AppMode {
        None,
        Start,
        Calibrating,
        Measure,
        Results,
        Debug,
        Update,
        Menu,
    }

    #[self_referencing]
    #[cfg(usb)]
    pub struct UsbDevices {
        bus: UsbBusAllocator<UsbBus<USB>>,

        #[borrows(bus)]
        #[covariant]
        pub serial: SerialPort<'this, UsbBus<USB>>,

        #[borrows(bus)]
        #[covariant]
        pub device: UsbDevice<'this, UsbBus<USB>>,
    }

    #[cfg(usb)]
    impl UsbDevices {
        pub fn make(bus: UsbBusAllocator<UsbBus<USB>>) -> Self {
            let usb = UsbDevicesBuilder {
                bus,
                device_builder: |bus| {
                    cortex_m::interrupt::free(|_cs| {
                        UsbDeviceBuilder::new(&bus, UsbVidPid(0x16c0, 0x27dd))
                            .strings(&[StringDescriptors::default()
                                .product("Shutter Speed Tester")
                                .manufacturer("inbox@null.page")])
                            .unwrap()
                            .device_class(usbd_serial::USB_CLASS_CDC)
                            .build()
                    })
                },
                serial_builder: |bus| cortex_m::interrupt::free(|_cs| SerialPort::new(bus)),
            }
            .build();

            unsafe { NVIC::unmask(Interrupt::OTG_FS) };

            usb
        }

        pub fn poll_serial(&mut self) -> bool {
            self.with_mut(|s| s.device.poll(&mut [s.serial]))
        }
    }

    static mut MEASUREMENT_BUFFER: RingBuffer = RingBuffer::new();

    #[cfg(usb)]
    type UsbDevicesImpl = UsbDevices;

    #[cfg(not(usb))]
    type UsbDevicesImpl = ();

    #[shared]
    struct Shared {
        transfer: config::DmaTransfer,
        adc_value: u16,
        sample_counter: Wrapping<u32>,
        app_mode: AppMode,
        calibration_state: CalibrationState,
        measurement: Measurement<'static, CycleCounterClock<{ hw::SYSCLK }>>,
        display: UnsafeCell<DisplayType>,
        usb_devices: UsbDevicesImpl,
        beep_sender: Sender<'static, Chirp, 1>,
        selected_menu_option: usize,
    }

    #[local]
    struct Local {
        adc_dma_buffer: Option<&'static mut u16>,
        timer: config::AdcTimerType,
        mode_button_pin: ErasedPin<Input>,
        measure_button_pin: ErasedPin<Input>,
        led_pin: ErasedPin<Output>,
        beeper: Beeper,
        rotary: RotaryEncoder<StandardMode, ErasedPin<Input>, ErasedPin<Input>>,
    }

    #[cfg(usb)]
    static mut USB_EP_MEMORY: [u32; 1024] = [0; 1024];

    #[init(local = [first_buffer: u16 = 0, _adc_dma_buffer: u16 = 0])]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        let mut dp: pac::Peripherals = cx.device;

        let gpio = AllGpio {
            a: dp.GPIOA.split(),
            b: dp.GPIOB.split(),
            c: dp.GPIOC.split(),
        };

        let mut led_pin = hw::led_pin!(gpio).into_push_pull_output();

        let mut backlight_pin = hw::display_backlight_pin!(gpio).into_push_pull_output();
        backlight_pin.set_low();

        // Workaround 1 enable prefetch
        // {
        //     dp.FLASH
        //         .acr
        //         .write(|w| w.prften().enabled().icen().enabled().dcen().enabled());
        // }

        // Workaround 2 AN4073 4.1 reduce ADC crosstalk
        // {
        //     dp.PWR.cr.write(|w| w.adcdc1().set_bit());
        // }

        // // Workaround 3 AN4073 4.1 reduce ADC crosstalk
        // {
        //     dp.SYSCFG.pmc.write(|x| x.adc1dc2().set_bit())
        // }

        cortex_m::asm::delay(10000);
        let mut syscfg = dp.SYSCFG.constrain();

        let clocks = config::setup_clocks!(dp);

        CYCCNTClock::<{ hw::SYSCLK }>::init(&mut cx.core.DCB, cx.core.DWT);

        let systick_token = create_systick_token!();
        Systick::start(cx.core.SYST, hw::SYSCLK, systick_token);

        let adc = config::setup_adc!(dp, gpio);
        let transfer = config::setup_adc_dma_transfer!(dp, adc, cx.local.first_buffer);
        let timer = config::setup_adc_timer!(cx.core, dp, &clocks);
        let mut delay = config::delay_timer!(dp).delay_us(&clocks);

        let mut display = {
            let mut dc_pin = hw::display_dc_pin!(gpio).into_push_pull_output();
            let mut rst_pin = hw::display_rst_pin!(gpio).into_push_pull_output();

            dc_pin.set_speed(Speed::VeryHigh);
            rst_pin.set_speed(Speed::VeryHigh);
            let spi = config::setup_display_spi!(dp, gpio, &clocks);

            Display::new(
                spi,
                dc_pin.erase(),
                rst_pin.erase(),
                backlight_pin.erase(),
                &mut delay,
            )
        };

        display.sneaky_clear(Rgb565::BLACK);
        display.backlight_on();

        let mut mode_button_pin = hw::mode_button_pin!(gpio).into_pull_down_input();
        mode_button_pin.make_interrupt_source(&mut syscfg);
        mode_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        mode_button_pin.enable_interrupt(&mut dp.EXTI);

        let mut measure_button_pin = hw::measure_button_pin!(gpio).into_pull_down_input();
        measure_button_pin.make_interrupt_source(&mut syscfg);
        measure_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        measure_button_pin.enable_interrupt(&mut dp.EXTI);

        #[cfg(usb)]
        usb_task::spawn().unwrap();
        led_pin.set_low();

        let display = UnsafeCell::new(display);

        #[cfg(usb)]
        let usb_bus = UsbBusType::new(
            USB {
                usb_global: dp.OTG_FS_GLOBAL,
                usb_device: dp.OTG_FS_DEVICE,
                usb_pwrclk: dp.OTG_FS_PWRCLK,
                pin_dm: hw::usb_dm_pin!(gpio).into(),
                pin_dp: hw::usb_dp_pin!(gpio).into(),
                hclk: clocks.hclk(),
            },
            unsafe { &mut USB_EP_MEMORY },
        );

        let beeper = config::setup_sound_pwm!(dp, gpio, &clocks);
        let (beep_tx, beep_rx) = make_channel!(Chirp, 1);
        beeper_task::spawn(beep_rx).unwrap();

        let (rotary_tx, rotary_rx) = make_channel!(EncoderDirection, 100);

        let rotary = RotaryEncoder::new(
            hw::rotary_dt_pin!(gpio).into_pull_up_input().erase(),
            hw::rotary_clk_pin!(gpio).into_pull_up_input().erase(),
        )
        .into_standard_mode();
        rotary_encoder_task::spawn(rotary_tx).unwrap();

        display_task::spawn(rotary_rx).unwrap();

        (
            Shared {
                transfer,
                adc_value: 0,
                sample_counter: Wrapping(0),
                app_mode: AppMode::Start,
                calibration_state: CalibrationState::Done(0),
                measurement: Measurement::new(
                    0,
                    unsafe { &mut MEASUREMENT_BUFFER },
                    hw::TRIGGER_THRESHOLD_LOW,
                    hw::TRIGGER_THRESHOLD_HIGH,
                ),
                display,
                #[cfg(usb)]
                usb_devices: UsbDevices::make(usb_bus),
                #[cfg(not(usb))]
                usb_devices: (),
                beep_sender: beep_tx,
                selected_menu_option: 0,
            },
            Local {
                adc_dma_buffer: Some(cx.local._adc_dma_buffer),
                timer,
                mode_button_pin: mode_button_pin.erase(),
                measure_button_pin: measure_button_pin.erase(),
                led_pin: led_pin.erase(),
                beeper,
                rotary,
            },
        )
    }

    #[task(local=[rotary], shared=[app_mode, selected_menu_option], priority=2)]
    async fn rotary_encoder_task(
        mut cx: rotary_encoder_task::Context,
        mut tx: Sender<'static, EncoderDirection, 100>,
    ) {
        fn step_back(shared: &mut rotary_encoder_task::SharedResources) {
            (&mut shared.app_mode, &mut shared.selected_menu_option).lock(
                |app_mode, selected_menu_option| match *app_mode {
                    AppMode::Start | AppMode::Calibrating | AppMode::Measure | AppMode::Debug => {
                        *app_mode = AppMode::Menu;
                    }
                    AppMode::Menu => {
                        if *selected_menu_option == 0 {
                            *app_mode = AppMode::Start;
                        } else {
                            *selected_menu_option -= 1;
                        }
                    }
                    _ => (),
                },
            );
        }
        fn step_forward(shared: &mut rotary_encoder_task::SharedResources) {
            (&mut shared.app_mode, &mut shared.selected_menu_option).lock(
                |app_mode, selected_menu_option| match *app_mode {
                    AppMode::Start | AppMode::Calibrating | AppMode::Measure | AppMode::Debug => {
                        *app_mode = AppMode::Menu;
                        *selected_menu_option = 0;
                    }
                    AppMode::Menu => {
                        if *selected_menu_option == MenuScreen::options_len() - 1 {
                            *app_mode = AppMode::Start;
                        } else {
                            *selected_menu_option += 1;
                        }
                    }
                    _ => (),
                },
            );
        }
        let encoder = cx.local.rotary;
        loop {
            encoder.update();
            match encoder.direction() {
                EncoderDirection::None => (),
                x => {
                    let d: isize = match x {
                        EncoderDirection::Anticlockwise => 1,
                        EncoderDirection::Clockwise => -1,
                        _ => 0,
                    };

                    (&mut cx.shared.app_mode, &mut cx.shared.selected_menu_option).lock(
                        |app_mode, selected_menu_option| match *app_mode {
                            AppMode::Start
                            | AppMode::Calibrating
                            | AppMode::Measure
                            | AppMode::Debug => {
                                *app_mode = AppMode::Menu;
                            }
                            AppMode::Menu => {
                                *selected_menu_option = (*selected_menu_option as isize
                                    + MenuScreen::options_len() as isize
                                    + d)
                                    as usize
                                    % MenuScreen::options_len();
                            }
                            _ => (),
                        },
                    );
                }
            }
            Systick::delay(10.millis()).await;
        }
    }

    #[task(local=[beeper], priority=5)]
    async fn beeper_task(cx: beeper_task::Context, mut beep_rx: Receiver<'static, Chirp, 1>) {
        let beeper = cx.local.beeper;
        while let Ok(chirp) = beep_rx.recv().await {
            match chirp {
                Chirp::Startup => {
                    // Remember
                    beeper.play(12 + -2, 250).await;
                    beeper.play(12 + 5, 250).await;
                    beeper.play(12 + 9, 250).await;
                    Systick::delay(2000.millis()).await;
                }
                Chirp::Button => {
                    beeper.note(9);
                    Systick::delay(50.millis()).await;
                    beeper.disable();
                }
                Chirp::Measuring => {
                    beeper.play(20, 100).await;
                }
                Chirp::Done => {
                    beeper.play(12 - 2, 100).await;
                    beeper.play(24 - 2, 100).await;
                }
            }
        }
    }

    // HWCONFIG
    #[task(binds = TIM2, shared = [transfer], local = [timer], priority = 3)]
    fn adcstart(mut cx: adcstart::Context) {
        cx.shared.transfer.lock(|transfer| {
            transfer.start(|adc| {
                adc.start_conversion();
            });
        });
        cx.local.timer.clear_flags(Flag::Update);
    }

    // HWCONFIG
    #[task(binds = EXTI1, shared = [app_mode, beep_sender], local=[mode_button_pin], priority = 4)]
    fn mode_button_press(mut cx: mode_button_press::Context) {
        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Button);
        });
        cx.shared.app_mode.lock(|app_mode| match app_mode {
            // _ => {
            //     *app_mode = AppMode::Update;
            // }
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

    // HWCONFIG
    #[task(binds = EXTI2, shared = [app_mode, beep_sender, selected_menu_option], local=[measure_button_pin, led_pin], priority = 4)]
    fn measure_button_press(mut cx: measure_button_press::Context) {
        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Button);
        });
        let selected_option = cx
            .shared
            .selected_menu_option
            .lock(|selected_menu_option| *selected_menu_option);
        cx.shared.app_mode.lock(|app_mode| match *app_mode {
            AppMode::Menu => match selected_option {
                0 => {
                    let _ = measure_task::spawn();
                    // *app_mode = AppMode::Start;
                }
                1 => {
                    *app_mode = AppMode::Debug;
                }
                2 => {}
                3 => {
                    *app_mode = AppMode::Update;
                }
                _ => (),
            },
            AppMode::Calibrating | AppMode::Measure => {
                *app_mode = AppMode::Start;
            }
            _ => {
                let _ = measure_task::spawn();
            }
        });
        cx.local.measure_button_pin.clear_interrupt_pending_bit();
    }

    // HWCONFIG
    #[task(binds = DMA2_STREAM0, shared = [transfer, adc_value, sample_counter, calibration_state, measurement], local = [adc_dma_buffer], priority = 3)]
    fn dma(cx: dma::Context) {
        let mut shared = cx.shared;
        let local = cx.local;

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

    #[task(shared=[app_mode, adc_value, calibration_state, measurement, beep_sender], priority=2)]
    async fn measure_task(mut cx: measure_task::Context) {
        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        cx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(hw::CALIBRATION_TIME_MS.millis()).await;

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Measuring);
        });

        let calibration_value = cx.shared.calibration_state.lock(|state| state.finish());
        cx.shared.measurement.lock(|measurement| {
            *measurement = Measurement::new(
                calibration_value,
                unsafe { &mut MEASUREMENT_BUFFER },
                hw::TRIGGER_THRESHOLD_LOW,
                hw::TRIGGER_THRESHOLD_HIGH,
            );
        });

        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Measure;
        });

        loop {
            if cx.shared.app_mode.lock(|app_mode| *app_mode) != AppMode::Measure {
                // Cancelled
                return;
            }

            if cx
                .shared
                .measurement
                .lock(|measurement| measurement.is_done())
            {
                break;
            }

            Systick::delay(100.millis()).await;
        }

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Done);
        });
        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Results;
        });
    }

    #[task(shared=[app_mode, calibration_state], priority=2)]
    async fn debug_task(mut cx: debug_task::Context) {
        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Calibrating;
        });
        cx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        Systick::delay(hw::CALIBRATION_TIME_MS.millis()).await;

        cx.shared.app_mode.lock(|app_mode| {
            *app_mode = AppMode::Debug;
        });
    }

    fn handle_usb_activity(_usb: &mut UsbDevicesImpl) {
        #[cfg(usb)]
        _usb.with_serial_mut(|serial| {
            let mut buf = [0; 64];
            match serial.read(&mut buf) {
                Ok(count) if count > 0 => {
                    serial.write(b"\r\n").unwrap();
                    serial.write(&buf[..count]).unwrap();
                }
                _ => {}
            }
        })
    }

    #[task(binds=OTG_FS, shared=[usb_devices])]
    fn usb_interrupt(cx: usb_interrupt::Context) {
        let mut usb = cx.shared.usb_devices;
        usb.lock(handle_usb_activity);
    }

    #[task(shared=[usb_devices], priority=1)]
    async fn usb_task(_cx: usb_task::Context) {
        #[cfg(usb)]
        {
            let mut usb = _cx.shared.usb_devices;
            loop {
                if !usb.lock(|usb| usb.poll_serial()) {
                    Systick::delay(10.millis()).await;
                }
                usb.lock(handle_usb_activity);
            }
        }
    }

    #[task(shared=[adc_value, sample_counter, app_mode, calibration_state, measurement, display, beep_sender, selected_menu_option], priority=1)]
    async fn display_task(
        mut cx: display_task::Context,
        encoder_rx: Receiver<'static, EncoderDirection, 100>,
    ) {
        // Only shared with the panic handler, which never returns
        let display = unsafe { cx.shared.display.lock(|d| &mut *d.get()) };

        BootScreen::default().draw_init(display).await;

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Startup);
        });

        let mut mode = AppMode::None;
        let mut screen: Screens<DisplayType, MipidsiError> = StartScreen::default().into();

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
                        screen = Screens::Start(StartScreen::default());
                    }
                    AppMode::Calibrating => {
                        screen = Screens::Calibration(CalibrationScreen::default());
                    }
                    AppMode::Measure => {
                        screen = Screens::Measurement(MeasurementScreen::default());
                    }
                    AppMode::Debug => {
                        screen = Screens::Debug(DebugScreen::new(
                            cx.shared.calibration_state.lock(core::mem::take).finish(),
                            hw::TRIGGER_THRESHOLD_LOW,
                            hw::TRIGGER_THRESHOLD_HIGH,
                            match hw::ADC_RESOLUTION {
                                Resolution::Six => 63,
                                Resolution::Eight => 255,
                                Resolution::Ten => 1023,
                                Resolution::Twelve => 4095,
                            },
                        ));
                    }
                    AppMode::Results => {
                        let calibration = cx.shared.calibration_state.lock(core::mem::take);
                        let result = cx
                            .shared
                            .measurement
                            .lock(|m| {
                                core::mem::replace(
                                    m,
                                    Measurement::new(
                                        Default::default(),
                                        unsafe { &mut MEASUREMENT_BUFFER },
                                        hw::TRIGGER_THRESHOLD_LOW,
                                        hw::TRIGGER_THRESHOLD_HIGH,
                                    ),
                                )
                            })
                            .take_result()
                            .unwrap();
                        screen = Screens::Results(ResultsScreen::new(calibration, result));
                    }
                    AppMode::Update => {
                        screen = Screens::Update(UpdateScreen::default());
                    }
                    AppMode::Menu => {
                        screen = Screens::Menu(MenuScreen::default());
                    }
                    AppMode::None => (),
                };
                screen.draw_init(display).await;
            }

            match screen {
                Screens::Debug(ref mut screen) => {
                    let adc_value = cx.shared.adc_value.lock(|adc_value| *adc_value);
                    screen.step(adc_value);
                }
                Screens::Menu(ref mut screen) => {
                    let selected_menu_option = cx
                        .shared
                        .selected_menu_option
                        .lock(|selected_menu_option| *selected_menu_option);
                    screen.position = selected_menu_option;
                }
                _ => (),
            }

            screen.draw_frame(display).await;
            display.step_fx();

            #[allow(clippy::single_match)]
            match screen {
                Screens::Update(_) => bootloader_api::reboot_into_bootloader(),
                _ => (),
            }

            let deadline = if matches!(screen, Screens::Debug(_)) {
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
