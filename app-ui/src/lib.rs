#![no_std]

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::pixelcolor::Rgb565;

mod config;
mod elements;
pub mod fonts;
mod format;
mod fx;
pub mod panic;
mod primitives;
mod screens;

pub use elements::*;
pub use screens::{
    BootScreen, CalibrationScreen, DebugScreen, MeasurementScreen, MenuScreen, ResultsScreen,
    Screen, Screens, StartScreen, UpdateScreen,
};

pub trait AppDrawTarget<E>: DrawTarget<Color = Rgb565, Error = E> {}
impl<E, D: DrawTarget<Color = Rgb565, Error = E>> AppDrawTarget<E> for D {}

pub use badge::draw_badge;
pub use fx::{FXParams, FX};
