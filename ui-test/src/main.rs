use std::f32::consts::PI;
use std::thread;
use std::time::{Duration, Instant};

use app_measurements::{
    CalibrationResult, CalibrationState, MeasurementResult, SamplingRate, TriggerThresholds,
};
use app_ui::panic::draw_panic_screen;
use app_ui::{
    BootScreen, CalibrationScreen, DebugScreen, DrawFrameContext, HintRefresh, MeasurementScreen,
    MenuScreen, NoAccessoryScreen, ResultsScreen, Screen, Screens, StartScreen, UpdateScreen,
};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::Pixel;
use embedded_graphics_simulator::sdl2::Keycode;
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use heapless::HistoryBuffer;

struct LiveDisplay<'a> {
    display: &'a mut SimulatorDisplay<Rgb565>,
    window: &'a mut Window,
}

impl HintRefresh for LiveDisplay<'_> {
    fn hint_refresh(&mut self) {
        self.window.update(self.display);
    }
}

impl OriginDimensions for LiveDisplay<'_> {
    fn size(&self) -> Size {
        self.display.size()
    }
}

impl DrawTarget for LiveDisplay<'_> {
    type Color = Rgb565;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.display.draw_iter(pixels)?;
        Ok(())
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let mut panic_visible = false;

    let mut display = SimulatorDisplay::new(Size::new(128, 160));

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    let mut w = Window::new("UI", &output_settings);

    let mut live_display = LiveDisplay {
        display: &mut display,
        window: &mut w,
    };

    let mut screen = Screens::Boot(BootScreen::default());
    screen.draw_init(&mut live_display).await;
    live_display.hint_refresh();

    let t_start = Instant::now();

    'outer: loop {
        screen
            .draw_frame(
                &mut live_display,
                DrawFrameContext {
                    animation_time_ms: t_start.elapsed().as_millis() as u32,
                },
            )
            .await;
        live_display.hint_refresh();

        if panic_visible {
            draw_panic_screen(
                &mut live_display,
                "TEST\nwarning: unused imports: `FXParams`, `FX`\n        --> src/main.rs:8:36",
            );
        }

        let mut need_init = false;
        for e in live_display.window.events() {
            match e {
                SimulatorEvent::Quit => {
                    break 'outer;
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    panic_visible = false;
                    match keycode {
                        Keycode::Num1 => {
                            screen = BootScreen::default().into();
                            need_init = true;
                        }
                        Keycode::Q => {
                            screen = StartScreen::default().into();
                            need_init = true;
                        }
                        Keycode::W => {
                            screen = CalibrationScreen::default().into();
                            need_init = true;
                        }
                        Keycode::E => {
                            screen = MeasurementScreen::default().into();
                            need_init = true;
                        }
                        Keycode::R => {
                            let mut sample_buffer = HistoryBuffer::new();
                            let size = sample_buffer.capacity();
                            let margin = 100;
                            let baseline = 127;

                            for _ in 0..margin {
                                sample_buffer.write(baseline);
                            }
                            for i in 0..size - margin * 2 {
                                sample_buffer.write(
                                    ((i as f32 / 300.0 * PI).sin() * 128.0) as u16 + baseline,
                                );
                            }
                            for _ in 0..margin {
                                sample_buffer.write(baseline);
                            }
                            screen = ResultsScreen::new(
                                CalibrationState::Done(CalibrationResult {
                                    average: 128,
                                    max: 160,
                                    min: 80,
                                }),
                                MeasurementResult {
                                    duration_micros: 125,
                                    integrated_duration_micros: 1000000 / 120,
                                    sample_buffer,
                                    samples_since_end: margin + 30,
                                    samples_since_start: size - margin - 30,
                                    sample_rate: SamplingRate::new(1),
                                },
                            )
                            .into();
                            need_init = true;
                        }
                        Keycode::T => {
                            screen = UpdateScreen::default().into();
                            need_init = true;
                        }
                        Keycode::Y => {
                            screen = MenuScreen::default().into();
                            need_init = true;
                        }
                        Keycode::U => {
                            panic_visible = true;
                        }
                        Keycode::I => {
                            let mut ds = DebugScreen::new(
                                CalibrationResult {
                                    average: 128,
                                    max: 160,
                                    min: 80,
                                },
                                TriggerThresholds {
                                    high_ratio: 1.2,
                                    low_ratio: 1.5,
                                    high_delta: 0,
                                    low_delta: 0,
                                },
                                128,
                            );
                            ds.step(55);
                            screen = ds.into();
                            need_init = true;
                        }
                        Keycode::O => {
                            screen = NoAccessoryScreen::default().into();
                            need_init = true;
                        }
                        Keycode::Up => match screen {
                            Screens::Menu(ref mut screen) => {
                                screen.position = screen.position.saturating_sub(1);
                            }
                            Screens::Debug(ref mut screen) => {
                                screen.step(screen.last_adc_value() + 5);
                            }
                            _ => (),
                        },
                        Keycode::Down => match screen {
                            Screens::Menu(ref mut screen) => {
                                screen.position = (screen.position + 1) % MenuScreen::options_len();
                            }
                            Screens::Debug(ref mut screen) => {
                                screen.step(screen.last_adc_value() - 5);
                            }
                            _ => (),
                        },
                        Keycode::Left | Keycode::Right => {
                            if let Screens::Menu(ref mut screen) = screen {
                                screen.sensitivity = (screen.sensitivity + 1) % 3;
                            }
                        }
                        _ => (),
                    }
                }
                _ => (),
            }
        }

        if need_init {
            screen.draw_init(&mut live_display).await;
            live_display.hint_refresh();
        }

        #[allow(clippy::single_match)]
        match screen {
            Screens::Debug(ref mut screen) => {
                screen.step(screen.last_adc_value());
            }
            Screens::Calibration(ref mut screen) => {
                screen.step(Some((t_start.elapsed().as_millis() / 10 % 100) as u8));
            }
            _ => (),
        }

        thread::sleep(Duration::from_millis(100));
    }
}
