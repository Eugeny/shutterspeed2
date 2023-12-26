use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
use rtic_monotonics::systick::Systick;
use rtic_monotonics::Monotonic;

use super::Screen;
use crate::display::AppDrawTarget;
use crate::ui::draw_badge;

pub struct MeasurementScreen {}

impl Screen for MeasurementScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        let origin = Point::new(display.bounding_box().size.width as i32 / 2, 100);
        draw_badge(display, origin, "  MEASURING  ", Rgb565::BLACK, Rgb565::RED).await;
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        let t = (Systick::now() - <Systick as rtic_monotonics::Monotonic>::ZERO).to_secs();
        let offsets = -2i32..2;
        let len = offsets.len() as u32;
        for (idx, dx) in offsets.enumerate() {
            let x = display.bounding_box().size.width as i32 / 2 + dx * 10;
            let y = 150;
            let color = if idx as u32 == t % len {
                Rgb565::RED
            } else {
                Rgb565::BLACK
            };
            display
                .fill_solid(
                    &Rectangle::with_center(Point::new(x, y), Size::new(5, 5)),
                    color,
                )
                .unwrap();
        }
    }
}
