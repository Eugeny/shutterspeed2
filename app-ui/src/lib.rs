#![no_std]

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;

mod badge;
mod chart;
mod config;
mod fonts;
mod format;
pub mod panic;
mod primitives;
mod ruler;
mod screens;

pub use screens::{
    BootScreen, CalibrationScreen, DebugScreen, MeasurementScreen, ResultsScreen, Screen, Screens,
    StartScreen, UpdateScreen,
};

pub trait AppDrawTarget<E>: DrawTarget<Color = Rgb565, Error = E> {}
impl<E, D: DrawTarget<Color = Rgb565, Error = E>> AppDrawTarget<E> for D {}

pub use badge::draw_badge;
