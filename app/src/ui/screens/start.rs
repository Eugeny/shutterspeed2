use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use rtic_monotonics::systick::Systick;
use rtic_monotonics::Monotonic;

use super::Screen;
use crate::display::AppDrawTarget;
use crate::ui::draw_badge;

pub struct StartScreen {}

impl Screen for StartScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_badge(
            display,
            Point::new(display.bounding_box().size.width as i32 / 2, 100),
            " READY ",
            Rgb565::CSS_PALE_GREEN,
            Rgb565::BLACK,
        )
        .await;
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        let t = (Systick::now() - <Systick as rtic_monotonics::Monotonic>::ZERO).to_millis() / 500;
        let color = if t % 2 == 0 {
            Rgb565::WHITE
        } else {
            Rgb565::BLACK
        };
        let center = display.bounding_box().center();
        display
            .fill_solid(
                &Rectangle::with_center(center + Point::new(0, 40), Size::new(10, 10)),
                color,
            )
            .unwrap();
    }
}
