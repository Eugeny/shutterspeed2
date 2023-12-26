mod calibration;
mod debug;
mod measurement;
mod results;
mod start;

pub use calibration::CalibrationScreen;
pub use debug::{DebugScreen, DebugUiState};
use enum_dispatch::enum_dispatch;
pub use measurement::MeasurementScreen;
pub use results::ResultsScreen;
pub use start::StartScreen;

use crate::display::AppDrawTarget;

#[enum_dispatch]
pub trait Screen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT);
    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT);
}
