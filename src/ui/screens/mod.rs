mod calibration;
mod debug;
mod measurement;
mod results;
mod start;

pub use calibration::CalibrationScreen;
pub use debug::{DebugScreen, DebugUiState};
pub use measurement::MeasurementScreen;
pub use results::{ResultsScreen, ResultsUiState};
pub use start::StartScreen;

use crate::display::AppDrawTarget;

pub trait Screen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT);
    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT);
}
