mod boot;
mod calibration;
mod debug;
mod measurement;
mod results;
mod start;
mod update;

use core::fmt::Debug;

pub use boot::BootScreen;
pub use calibration::CalibrationScreen;
pub use debug::DebugScreen;
use enum_dispatch::enum_dispatch;
pub use measurement::MeasurementScreen;
pub use results::ResultsScreen;
pub use start::StartScreen;
pub use update::UpdateScreen;

use crate::AppDrawTarget;

#[allow(async_fn_in_trait)]
#[enum_dispatch(Screens<DT, E>)]
pub trait Screen<DT: AppDrawTarget<E>, E: Debug> {
    async fn draw_init(&mut self, display: &mut DT);
    async fn draw_frame(&mut self, display: &mut DT);
}

#[allow(clippy::large_enum_variant)]
#[enum_dispatch]
pub enum Screens<DT: AppDrawTarget<E>, E: Debug> {
    Boot(BootScreen<DT, E>),
    Start(StartScreen<DT, E>),
    Calibration(CalibrationScreen<DT, E>),
    Measurement(MeasurementScreen<DT, E>),
    Debug(DebugScreen<DT, E>),
    Results(ResultsScreen<DT, E>),
    Update(UpdateScreen<DT, E>),
}
