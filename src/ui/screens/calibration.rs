use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};

use super::Screen;
use crate::display::AppDrawTarget;
use crate::ui::draw_badge;

pub struct CalibrationScreen {}

impl Screen for CalibrationScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        let origin = Point::new(display.bounding_box().size.width as i32 / 2, 100);
        draw_badge(
            display,
            origin,
            " CALIBRATING ",
            Rgb565::BLACK,
            Rgb565::YELLOW,
        )
        .await;
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, _display: &mut DT) {}
}
