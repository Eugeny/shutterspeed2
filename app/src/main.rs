#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]
#![feature(iter_array_chunks)]
#![feature(sync_unsafe_cell)]

mod display;
mod panic;
mod sound;

extern "C" {
    static mut HEAP: u32;
}

// HWCONFIG
#[rtic::app(device = hal::pac, dispatchers = [SPI2, SPI3, SPI4, I2C1_EV])]
mod app {
    use core::cell::UnsafeCell;
    use core::num::Wrapping;
    use core::panic;
    #[cfg(feature = "usb")]
    use core::ptr::addr_of_mut;

    use app_measurements::{CalibrationResult, CalibrationState, CycleCounterClock, Measurement};
    use app_ui::{
        BootScreen, CalibrationScreen, DebugScreen, DrawFrameContext, MeasurementScreen,
        MenuScreen, NoAccessoryScreen, ResultsScreen, Screen, Screens, StartScreen, UpdateScreen,
    };
    use config::{self as hw, hal, AllGpio};
    #[cfg(feature = "usb")]
    use cortex_m::peripheral::NVIC;
    use cortex_m_microclock::CYCCNTClock;
    use embedded_alloc::Heap;
    use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
    use fugit::ExtU32;
    use hal::adc::config::Resolution;
    use hal::gpio::{Edge, ErasedPin, Input, Output};
    #[cfg(feature = "usb")]
    use hal::otg_fs::UsbBusType;
    use hal::otg_fs::{UsbBus, USB};
    use hal::pac;
    use hal::prelude::*;
    use hal::timer::Flag;
    #[cfg(feature = "usb")]
    use heapless::String;
    use mipidsi::error::Error as MipidsiError;
    use ouroboros::self_referencing;
    use rotary_encoder_embedded::standard::StandardMode;
    use rotary_encoder_embedded::{Direction, RotaryEncoder};
    use rtic_monotonics::systick::Systick;
    use rtic_monotonics::{create_systick_token, Monotonic};
    use rtic_sync::channel::{Receiver, Sender};
    use rtic_sync::make_channel;
    #[cfg(feature = "usb")]
    use stm32f4xx_hal::pac::Interrupt;
    #[cfg(feature = "usb")]
    use ufmt::uwrite;
    use usb_device::class_prelude::UsbBusAllocator;
    use usb_device::device::UsbDevice;
    #[cfg(feature = "usb")]
    use usb_device::device::{StringDescriptors, UsbDeviceBuilder, UsbVidPid};
    use usbd_serial::SerialPort;

    use crate::display::Display;
    use crate::panic::set_panic_display_ref;
    use crate::sound::{BeeperExt, Chirp};

    pub type DisplayType = Display<config::DisplaySpiType>;

    config::beeper_type!();

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum AppModeInner {
        None,
        Start,
        Calibrating,
        Measure,
        Results,
        Debug,
        Update,
        NoAccessory,
        Menu,
    }

    pub struct AppMode {
        inner: AppModeInner,
        acc_idle_pin: ErasedPin<Output>,
    }

    impl AppMode {
        pub fn new(acc_idle_pin: ErasedPin<Output>) -> Self {
            AppMode {
                inner: AppModeInner::Start,
                acc_idle_pin,
            }
        }

        pub fn get(&self) -> AppModeInner {
            self.inner
        }

        pub fn set(&mut self, mode: AppModeInner) {
            self.inner = mode;
            match mode {
                AppModeInner::Calibrating | AppModeInner::Measure | AppModeInner::Debug => {
                    self.acc_idle_pin.set_low()
                }
                _ => self.acc_idle_pin.set_high(),
            }
        }
    }

    #[self_referencing]
    pub struct UsbDevices {
        bus: UsbBusAllocator<UsbBus<USB>>,

        #[borrows(bus)]
        #[covariant]
        pub serial: SerialPort<'this, UsbBus<USB>>,

        #[borrows(bus)]
        #[covariant]
        pub device: UsbDevice<'this, UsbBus<USB>>,
    }

    #[cfg(feature = "usb")]
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

    pub struct UsbDevicesStub;

    #[cfg(feature = "usb")]
    type UsbDevicesImpl = UsbDevices;

    #[cfg(not(feature = "usb"))]
    type UsbDevicesImpl = UsbDevicesStub;

    macro_rules! serial_log {
        ($usb_devices: expr, $slice: expr) => {
            #[cfg(feature = "usb")]
            $usb_devices.lock(|usb| {
                usb.with_serial_mut(|serial| {
                    let _ = serial.write($slice);
                })
            });
        };
    }

    #[shared]
    struct Shared {
        transfer: config::DmaTransfer,
        adc_value: u16,
        sample_counter: Wrapping<u32>,
        app_mode: AppMode,
        calibration_state: CalibrationState,
        calibration_result: Option<CalibrationResult>,
        measurement: Measurement<CycleCounterClock<{ hw::SYSCLK }>>,
        display: UnsafeCell<DisplayType>,
        beep_sender: Sender<'static, Chirp, 1>,
        selected_menu_option: usize,
        usb_devices: UsbDevicesImpl,
    }

    #[local]
    struct Local {
        adc_dma_buffer: Option<&'static mut u16>,
        timer: config::AdcTimerType,
        measure_button_pin: ErasedPin<Input>,
        led_pin: ErasedPin<Output>,
        beeper: Beeper,
        rotary: RotaryEncoder<StandardMode, ErasedPin<Input>, ErasedPin<Input>>,
        measurement_button_last_pressed: <Systick as Monotonic>::Instant,
        acc_sense_pin: ErasedPin<Input>,
        debug_calibration_channel_sender: Sender<'static, CalibrationResult, 1>,
        debug_calibration_channel_receiver: Receiver<'static, CalibrationResult, 1>,
        measurement_calibration_channel_sender: Sender<'static, CalibrationResult, 1>,
        measurement_calibration_channel_receiver: Receiver<'static, CalibrationResult, 1>,
    }

    #[cfg(feature = "usb")]
    static mut USB_EP_MEMORY: [u32; 1024] = [0; 1024];

    #[global_allocator]
    static HEAP: Heap = Heap::empty();

    #[init(local = [first_buffer: u16 = 0, _adc_dma_buffer: u16 = 0])]
    fn init(mut cx: init::Context) -> (Shared, Local) {
        {
            const HEAP_SIZE: usize = 1024;
            // unsafe { HEAP.init(super::HEAP as usize, HEAP_SIZE) }
        }

        let mut dp: pac::Peripherals = cx.device;

        let gpio = AllGpio {
            a: dp.GPIOA.split(),
            b: dp.GPIOB.split(),
            c: dp.GPIOC.split(),
        };

        let mut led_pin = hw::led_pin!(gpio).into_push_pull_output();

        let mut backlight_pin = hw::display_backlight_pin!(gpio).into_push_pull_output();
        backlight_pin.set_low();

        // HWCONFIG
        // Workaround 1 enable prefetch
        {
            dp.FLASH
                .acr
                .write(|w| w.prften().enabled().icen().enabled().dcen().enabled());
        }

        // HWCONFIG
        // Workaround 2 AN4073 4.1 reduce ADC crosstalk
        // {
        //     dp.PWR.cr.write(|w| w.adcdc1().set_bit());
        // }

        // HWCONFIG
        // Workaround 3 AN4073 4.1 reduce ADC crosstalk
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
        let transfer = config::setup_adc_dma_transfer!(cx.core, dp, adc, cx.local.first_buffer);
        let timer = config::setup_adc_timer!(dp, &clocks);
        let mut delay = config::delay_timer!(dp).delay_us(&clocks);

        let mut display = {
            Display::new(
                hw::setup_display!(dp, gpio, &clocks, &mut delay).unwrap(),
                backlight_pin.erase(),
            )
        };

        display.sneaky_clear(Rgb565::BLACK);
        display.backlight_on();

        let mut measure_button_pin = hw::measure_button_pin!(gpio).into_pull_down_input();
        measure_button_pin.make_interrupt_source(&mut syscfg);
        measure_button_pin.trigger_on_edge(&mut dp.EXTI, Edge::Rising);
        measure_button_pin.enable_interrupt(&mut dp.EXTI);

        let acc_sense_pin = hw::accessory_sense_pin!(gpio).into_pull_down_input();
        let mut acc_idle_pin = hw::accessory_idle_signal!(gpio).into_push_pull_output();
        acc_idle_pin.set_high();

        led_pin.set_low();

        let display = UnsafeCell::new(display);

        #[cfg(feature = "usb")]
        let usb_bus = UsbBusType::new(
            USB {
                usb_global: dp.OTG_FS_GLOBAL,
                usb_device: dp.OTG_FS_DEVICE,
                usb_pwrclk: dp.OTG_FS_PWRCLK,
                pin_dm: hw::usb_dm_pin!(gpio).into(),
                pin_dp: hw::usb_dp_pin!(gpio).into(),
                hclk: clocks.hclk(),
            },
            unsafe { &mut *addr_of_mut!(USB_EP_MEMORY) },
        );

        let beeper = config::setup_sound_pwm!(dp, gpio, &clocks);
        let (beep_tx, beep_rx) = make_channel!(Chirp, 1);
        beeper_task::spawn(beep_rx).unwrap();

        #[cfg(feature = "usb")]
        usb_task::spawn().unwrap();

        let rotary = RotaryEncoder::new(
            hw::rotary_dt_pin!(gpio).into_pull_up_input().erase(),
            hw::rotary_clk_pin!(gpio).into_pull_up_input().erase(),
        )
        .into_standard_mode();
        rotary_encoder_task::spawn().unwrap();

        display_task::spawn().unwrap();
        acc_sense_task::spawn().unwrap();

        let (debug_calibration_channel_sender, debug_calibration_channel_receiver) =
            make_channel!(CalibrationResult, 1);
        let (measurement_calibration_channel_sender, measurement_calibration_channel_receiver) =
            make_channel!(CalibrationResult, 1);

        (
            Shared {
                transfer,
                adc_value: 0,
                sample_counter: Wrapping(0),
                app_mode: AppMode::new(acc_idle_pin.erase()),
                calibration_state: CalibrationState::default(),
                calibration_result: None,
                measurement: Measurement::new(CalibrationResult::default(), hw::TRIGGER_THRESHOLDS),
                display,
                #[cfg(feature = "usb")]
                usb_devices: UsbDevices::make(usb_bus),
                #[cfg(not(feature = "usb"))]
                usb_devices: UsbDevicesStub,
                beep_sender: beep_tx,
                selected_menu_option: 0,
            },
            Local {
                adc_dma_buffer: Some(cx.local._adc_dma_buffer),
                timer,
                measure_button_pin: measure_button_pin.erase(),
                led_pin: led_pin.erase(),
                beeper,
                rotary,
                measurement_button_last_pressed: Systick::now(),
                acc_sense_pin: acc_sense_pin.erase(),
                debug_calibration_channel_sender,
                debug_calibration_channel_receiver,
                measurement_calibration_channel_sender,
                measurement_calibration_channel_receiver,
            },
        )
    }

    #[task(local=[rotary], shared=[app_mode, selected_menu_option, usb_devices], priority=2)]
    async fn rotary_encoder_task(mut cx: rotary_encoder_task::Context) {
        let encoder = cx.local.rotary;
        loop {
            encoder.update();
            match encoder.direction() {
                Direction::None => (),
                x => {
                    serial_log!(cx.shared.usb_devices, b"turned\r\n");

                    let d: isize = match x {
                        Direction::Clockwise => 1,
                        Direction::Anticlockwise => -1,
                        _ => 0,
                    };

                    (&mut cx.shared.app_mode, &mut cx.shared.selected_menu_option).lock(
                        |app_mode, selected_menu_option| match app_mode.get() {
                            AppModeInner::Start
                            | AppModeInner::Calibrating
                            | AppModeInner::Measure
                            | AppModeInner::Results
                            | AppModeInner::Debug => {
                                app_mode.set(AppModeInner::Menu);
                            }
                            AppModeInner::Menu => {
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
            Systick::delay(1.millis()).await;
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
                    beeper.play(24 - 2, 100).await;
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
    #[task(binds = EXTI2, shared = [app_mode, beep_sender, selected_menu_option], local=[measure_button_pin, measurement_button_last_pressed, led_pin], priority = 4)]
    fn measure_button_press(mut cx: measure_button_press::Context) {
        if (Systick::now() - *cx.local.measurement_button_last_pressed).to_millis() < 100 {
            cx.local.measure_button_pin.clear_interrupt_pending_bit();
            return;
        }
        *cx.local.measurement_button_last_pressed = Systick::now();

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Button);
        });
        let selected_option = cx
            .shared
            .selected_menu_option
            .lock(|selected_menu_option| *selected_menu_option);
        cx.shared.app_mode.lock(|app_mode| match app_mode.get() {
            AppModeInner::Calibrating | AppModeInner::Measure | AppModeInner::Debug => {
                app_mode.set(AppModeInner::Start);
            }
            AppModeInner::Menu => match selected_option {
                0 => {
                    let _ = measure_task::spawn();
                }
                1 => {
                    let _ = debug_task::spawn();
                }
                2 => {
                    app_mode.set(AppModeInner::Update);
                }
                _ => (),
            },
            AppModeInner::Update | AppModeInner::None | AppModeInner::NoAccessory => (),
            AppModeInner::Start | AppModeInner::Results => {
                let _ = measure_task::spawn();
            }
        });
        cx.local.measure_button_pin.clear_interrupt_pending_bit();
    }

    // HWCONFIG
    #[task(binds = DMA2_STREAM0, shared = [transfer, adc_value, sample_counter, calibration_state, measurement], local = [adc_dma_buffer], priority = 5)]
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
                    if let CalibrationState::InProgress { .. } = calibration_state {
                        calibration_state.step(value)
                    } else {
                        measurement.step(value);
                    }
                    *adc_value = value;
                    *sample_counter += Wrapping(1);
                },
            );
    }

    #[task(shared=[app_mode], local=[acc_sense_pin], priority=2)]
    async fn acc_sense_task(mut cx: acc_sense_task::Context) {
        let mut last_state = cx.local.acc_sense_pin.is_high();
        // TODO use adc
        loop {
            let state = cx.local.acc_sense_pin.is_high();
            Systick::delay(250.millis()).await;

            if !state {
                cx.shared.app_mode.lock(|app_mode| {
                    app_mode.set(AppModeInner::NoAccessory);
                });
            }

            if state != last_state {
                last_state = state;

                if state {
                    cx.shared.app_mode.lock(|app_mode| {
                        app_mode.set(AppModeInner::Start);
                    });
                }
            }
        }
    }

    #[task(shared = [app_mode, calibration_result, calibration_state], priority = 3)]
    async fn calibration_task(
        mut cx: calibration_task::Context,
        mut sender: Sender<'static, CalibrationResult, 1>,
    ) {
        cx.shared.calibration_result.lock(|r| *r = None);

        cx.shared.app_mode.lock(|app_mode| {
            app_mode.set(AppModeInner::Calibrating);
        });

        // Let the system settle a bit
        Systick::delay(250.millis()).await;

        cx.shared.calibration_state.lock(|calibration_state| {
            calibration_state.begin();
        });

        let calibration_result = loop {
            Systick::delay(100.millis()).await;
            let result = cx.shared.calibration_state.lock(|state| match state {
                CalibrationState::InProgress { .. } => None,
                CalibrationState::Done(result) => Some(result.clone()),
            });
            if let Some(result) = result {
                break result;
            }
        };

        sender.send(calibration_result).await.unwrap();
    }

    #[task(
        shared=[app_mode, adc_value, measurement, beep_sender, usb_devices],
        local=[measurement_calibration_channel_receiver, measurement_calibration_channel_sender],
        priority=2,
    )]
    async fn measure_task(mut cx: measure_task::Context) {
        #[cfg(feature = "usb")]
        let mut usb_devices = cx.shared.usb_devices;

        calibration_task::spawn(cx.local.measurement_calibration_channel_sender.clone()).unwrap();
        let result = cx
            .local
            .measurement_calibration_channel_receiver
            .recv()
            .await
            .unwrap();

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Measuring);
        });

        #[cfg(feature = "usb")]
        {
            let mut s = String::<128>::default();
            uwrite!(s, "Calibrated to: {}\r\n", result).unwrap();
            serial_log!(usb_devices, s.as_bytes());
        }

        cx.shared.measurement.lock(|measurement| {
            *measurement = Measurement::new(result, hw::TRIGGER_THRESHOLDS);
        });

        cx.shared.app_mode.lock(|app_mode| {
            app_mode.set(AppModeInner::Measure);
        });

        loop {
            if cx.shared.app_mode.lock(|app_mode| app_mode.get()) != AppModeInner::Measure {
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

        #[cfg(feature = "usb")]
        cx.shared.measurement.lock(|measurement| {
            if let Some(result) = measurement.result() {
                serial_log!(usb_devices, b"Result: \r\n");

                let mut s = String::<128>::default();
                uwrite!(s, "Raw start-end time: {} us\r\n", result.duration_micros).unwrap();
                serial_log!(usb_devices, s.as_bytes());

                let mut s = String::<128>::default();
                uwrite!(
                    s,
                    "Integrated time: {} us\r\n",
                    result.integrated_duration_micros
                )
                .unwrap();
                serial_log!(usb_devices, s.as_bytes());

                let mut s = String::<128>::default();
                uwrite!(
                    s,
                    "Sample rate at the end: 1/{}\r\n",
                    result.sample_rate.divisor()
                )
                .unwrap();
                serial_log!(usb_devices, s.as_bytes());

                let mut s = String::<128>::default();
                uwrite!(s, "Samples since start: {}\r\n", result.samples_since_start).unwrap();
                serial_log!(usb_devices, s.as_bytes());

                let mut s = String::<128>::default();
                uwrite!(s, "Samples since end: {}\r\n", result.samples_since_end).unwrap();
                serial_log!(usb_devices, s.as_bytes());

                let l = result.sample_buffer.len();
                for (index, item) in result.sample_buffer.oldest_ordered().enumerate() {
                    if index == l - result.samples_since_end {
                        serial_log!(usb_devices, b"** end **\r\n");
                    }

                    let mut s = String::<128>::default();
                    uwrite!(s, "- {}\r\n", item).unwrap();
                    serial_log!(usb_devices, s.as_bytes());

                    if index == l - result.samples_since_start {
                        serial_log!(usb_devices, b"** start **\r\n");
                    }
                }

                serial_log!(usb_devices, b"\r\n");
            }
        });

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Done);
        });
        cx.shared.app_mode.lock(|app_mode| {
            app_mode.set(AppModeInner::Results);
        });
    }

    #[task(
        shared=[app_mode, calibration_result],
        local=[debug_calibration_channel_sender, debug_calibration_channel_receiver],
        priority=2
    )]
    async fn debug_task(mut cx: debug_task::Context) {
        calibration_task::spawn(cx.local.debug_calibration_channel_sender.clone()).unwrap();
        let result = cx
            .local
            .debug_calibration_channel_receiver
            .recv()
            .await
            .unwrap();

        cx.shared
            .calibration_result
            .lock(|calibration_result| *calibration_result = Some(result));

        cx.shared.app_mode.lock(|app_mode| {
            app_mode.set(AppModeInner::Debug);
        });
    }

    #[cfg(feature = "usb")]
    fn handle_usb_activity(_usb: &mut UsbDevicesImpl) {
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
    fn usb_interrupt(_cx: usb_interrupt::Context) {
        #[cfg(feature = "usb")]
        {
            let mut usb = _cx.shared.usb_devices;
            usb.lock(handle_usb_activity);
        }
    }

    #[task(shared=[usb_devices], priority=1)]
    async fn usb_task(_cx: usb_task::Context) {
        #[cfg(feature = "usb")]
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

    #[task(shared=[adc_value, sample_counter, app_mode, calibration_state, calibration_result, measurement, display, beep_sender, selected_menu_option], priority=1)]
    async fn display_task(mut cx: display_task::Context) {
        // Only shared with the panic handler, which never returns
        let display = unsafe { cx.shared.display.lock(|d| &mut *d.get()) };

        BootScreen::default().draw_init(display).await;

        cx.shared.beep_sender.lock(|beep_sender| {
            let _ = beep_sender.try_send(Chirp::Startup);
        });

        let mut mode = AppModeInner::None;
        let mut screen: Screens<DisplayType, MipidsiError> = StartScreen::default().into();

        loop {
            if let Some(changed_mode) = cx.shared.app_mode.lock(|app_mode| {
                if app_mode.get() != mode {
                    mode = app_mode.get();
                    return Some(mode);
                }
                None
            }) {
                match changed_mode {
                    AppModeInner::Start => {
                        screen = Screens::Start(StartScreen::default());
                    }
                    AppModeInner::Calibrating => {
                        screen = Screens::Calibration(CalibrationScreen::default());
                    }
                    AppModeInner::Measure => {
                        screen = Screens::Measurement(MeasurementScreen::default());
                    }
                    AppModeInner::Debug => {
                        screen = Screens::Debug(DebugScreen::new(
                            cx.shared.calibration_result.lock(Option::take).unwrap(),
                            hw::TRIGGER_THRESHOLDS,
                            match hw::ADC_RESOLUTION {
                                Resolution::Six => 63,
                                Resolution::Eight => 255,
                                Resolution::Ten => 1023,
                                Resolution::Twelve => 4095,
                            },
                        ));
                    }
                    AppModeInner::Results => {
                        let calibration = cx.shared.calibration_state.lock(core::mem::take);
                        let result = cx
                            .shared
                            .measurement
                            .lock(|m| {
                                core::mem::replace(
                                    m,
                                    Measurement::new(CalibrationResult::default(), hw::TRIGGER_THRESHOLDS),
                                )
                            })
                            .take_result()
                            .unwrap();
                        screen = Screens::Results(ResultsScreen::new(calibration, result));
                    }
                    AppModeInner::Update => {
                        screen = Screens::Update(UpdateScreen::default());
                    }
                    AppModeInner::Menu => {
                        screen = Screens::Menu(MenuScreen::default());
                    }
                    AppModeInner::NoAccessory => {
                        screen = Screens::NoAccessory(NoAccessoryScreen::default());
                    }
                    AppModeInner::None => (),
                };
                screen.draw_init(display).await;
            }

            match screen {
                Screens::Debug(ref mut screen) => {
                    let adc_value = cx.shared.adc_value.lock(|adc_value| *adc_value);
                    screen.step(adc_value);
                }
                Screens::Calibration(ref mut screen) => {
                    let progress = cx.shared.calibration_state.lock(|c| c.progress());
                    screen.step(progress);
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

            screen
                .draw_frame(
                    display,
                    DrawFrameContext {
                        animation_time_ms: (Systick::now()
                            - <Systick as rtic_monotonics::Monotonic>::ZERO)
                            .to_millis(),
                    },
                )
                .await;
            display.step_fx();

            #[allow(clippy::single_match)]
            match screen {
                Screens::Update(_) => bootloader_api::reboot_into_bootloader(),
                _ => (),
            }

            let delay = match mode {
                AppModeInner::Debug => 10.millis(),
                AppModeInner::Calibrating => 10.millis(),
                AppModeInner::Measure => 500.millis(),
                _ => 25.millis(),
            };
            Systick::delay(delay).await;
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
