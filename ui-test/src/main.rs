use std::thread;
use std::time::Duration;

use app_measurements::{CalibrationState, MeasurementResult};
use app_ui::{
    BootScreen, CalibrationScreen, MeasurementScreen, ResultsScreen, Screen, Screens, StartScreen,
    UpdateScreen,
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

    let mut screen = Screens::Boot(BootScreen::default());
    screen.draw_init(&mut display).await;

    let output_settings = OutputSettingsBuilder::new().scale(2).build();
    let mut w = Window::new("UI", &output_settings);

    'outer: loop {
        screen.draw_frame(&mut display).await;
        w.update(&display);

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
                            screen = ResultsScreen::new(
                                CalibrationState::Done(128),
                                MeasurementResult {
                                    duration_micros: 125,
                                    integrated_duration_micros: 100,
                                    sample_buffer: HistoryBuffer::new_with(0),
                                    samples_since_end: 100,
                                    samples_since_start: 400,
                                },
                            )
                            .into()
                        }
                        Keycode::Y => screen = UpdateScreen::default().into(),
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
