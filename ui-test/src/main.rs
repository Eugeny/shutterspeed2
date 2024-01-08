use std::f32::consts::PI;
use std::thread;
use std::time::Duration;

use app_measurements::{CalibrationState, MeasurementResult};
use app_ui::{
    BootScreen, CalibrationScreen, FXParams, MeasurementScreen, ResultsScreen, Screen, Screens,
    StartScreen, UpdateScreen, FX,
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
    let mut display = SimulatorDisplay::new(Size::new(240, 320));

    let mut display = FX::new(&mut display, FXParams::default());

    let mut screen = Screens::Boot(BootScreen::default());
    screen.draw_init(&mut display).await;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    let mut w = Window::new("UI", &output_settings);

    'outer: loop {
        screen.draw_frame(&mut display).await;
        w.update(display.inner());
        display.step_params();

        for e in w.events() {
            match e {
                SimulatorEvent::Quit => {
                    break 'outer;
                }
                SimulatorEvent::KeyUp { keycode, .. } => {
                    match keycode {
                        Keycode::Q => screen = StartScreen::default().into(),
                        Keycode::W => screen = CalibrationScreen::default().into(),
                        Keycode::E => screen = MeasurementScreen::default().into(),
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
                                    integrated_duration_micros: 1000000 / 53,
                                    sample_buffer,
                                    samples_since_end: margin + 30,
                                    samples_since_start: size - margin - 30,
                                },
                            )
                            .into()
                        }
                        Keycode::T => screen = UpdateScreen::default().into(),
                        _ => (),
                    }
                    screen.draw_init(&mut display).await;
                }
                _ => (),
            }
        }

        thread::sleep(Duration::from_millis(20));
    }
}
