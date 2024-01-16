use std::f32::consts::PI;
use std::thread;
use std::time::Duration;

use app_measurements::{CalibrationState, MeasurementResult};
use app_ui::panic::draw_panic_screen;
use app_ui::{
    BootScreen, CalibrationScreen, DebugScreen, FXParams, MeasurementScreen, MenuScreen,
    ResultsScreen, Screen, Screens, StartScreen, UpdateScreen, FX,
};
use embedded_graphics::geometry::Size;
use embedded_graphics_simulator::sdl2::Keycode;
use embedded_graphics_simulator::{
    OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window,
};
use heapless::HistoryBuffer;

#[tokio::main(flavor = "current_thread")]
async fn main() {
    // Create a simulated display with the dimensions of the text box.
    let mut display = SimulatorDisplay::new(Size::new(128, 160));

    let mut panic_visible = false;

    // let mut display = FX::new(&mut display, FXParams::default());

    let mut screen = Screens::Boot(BootScreen::default());
    screen.draw_init(&mut display).await;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    let mut w = Window::new("UI", &output_settings);

    'outer: loop {
        screen.draw_frame(&mut display).await;
        // w.update(display.inner());
        // display.step_params();

        if panic_visible {
            draw_panic_screen(
                &mut display,
                "TEST\nwarning: unused imports: `FXParams`, `FX`\n        --> src/main.rs:8:36",
            );
        }
        w.update(&display);

        for e in w.events() {
            match e {
                SimulatorEvent::Quit => {
                    break 'outer;
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    panic_visible = false;
                    match keycode {
                        Keycode::Q => {
                            screen = StartScreen::default().into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::W => {
                            screen = CalibrationScreen::default().into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::E => {
                            screen = MeasurementScreen::default().into();
                            screen.draw_init(&mut display).await;
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
                                CalibrationState::Done(128),
                                MeasurementResult {
                                    duration_micros: 125,
                                    integrated_duration_micros: 1000000 / 120,
                                    sample_buffer,
                                    samples_since_end: margin + 30,
                                    samples_since_start: size - margin - 30,
                                },
                            )
                            .into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::T => {
                            screen = UpdateScreen::default().into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::Y => {
                            screen = MenuScreen::default().into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::U => {
                            panic_visible = true;
                        }
                        Keycode::I => {
                            let mut ds =DebugScreen::new(50, 1.2, 1.5, 128);
                            ds.step(55);
                            screen = ds.into();
                            screen.draw_init(&mut display).await;
                        }
                        Keycode::Up => {
                            if let Screens::Menu(ref mut screen) = screen {
                                screen.position = screen.position.saturating_sub(1);
                            }
                        }
                        Keycode::Down => {
                            if let Screens::Menu(ref mut screen) = screen {
                                screen.position = (screen.position + 1) % MenuScreen::options_len();
                            }
                        }
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

        thread::sleep(Duration::from_millis(20));
    }
}
