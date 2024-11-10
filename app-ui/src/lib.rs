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
mod util;

pub use elements::*;
pub use screens::{
    BootScreen, CalibrationScreen, DebugScreen, DrawFrameContext, MeasurementScreen, MenuScreen,
    NoAccessoryScreen, ResultsScreen, Screen, Screens, StartScreen, UpdateScreen,
};

pub trait HintRefresh {
    fn hint_refresh(&mut self);
}

pub trait AppDrawTarget<E>: DrawTarget<Color = Rgb565, Error = E> + HintRefresh {}
impl<E, D: DrawTarget<Color = Rgb565, Error = E> + HintRefresh> AppDrawTarget<E> for D {}

pub use badge::draw_badge;
pub use fx::{FXParams, FX};
