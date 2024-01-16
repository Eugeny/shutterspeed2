use core::fmt::Debug;

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};

use super::Screen;
use crate::{draw_badge, AppDrawTarget};

pub struct CalibrationScreen<DT, E> {
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for CalibrationScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_badge(
            display,
            display.bounding_box().center() - Point::new(0, 30),
            " CALIBRATING ",
            Rgb565::BLACK,
            Rgb565::YELLOW,
        )
        .await;
    }

    async fn draw_frame(&mut self, _display: &mut DT) {}
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for CalibrationScreen<DT, E> {
    fn default() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}
