use core::fmt::Debug;

use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::systick::Systick;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::Monotonic;

use super::Screen;
use crate::{draw_badge, AppDrawTarget};

pub struct StartScreen<DT, E> {
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for StartScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_badge(
            display,
            display.bounding_box().center() - Point::new(0, 30),
            " READY ",
            Rgb565::CSS_PALE_GREEN,
            Rgb565::BLACK,
        )
        .await;
    }

    async fn draw_frame(&mut self, display: &mut DT) {
        #[cfg(feature = "cortex-m")]
        let t = (Systick::now() - <Systick as rtic_monotonics::Monotonic>::ZERO).to_millis() / 500;
        #[cfg(not(feature = "cortex-m"))]
        let t = 0;
        let color = if t % 2 == 0 {
            Rgb565::WHITE
        } else {
            Rgb565::BLACK
        };
        let center = display.bounding_box().center();
        display
            .fill_solid(
                &Rectangle::with_center(center + Point::new(0, 10), Size::new(10, 10)),
                color,
            )
            .unwrap();
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for StartScreen<DT, E> {
    fn default() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}
