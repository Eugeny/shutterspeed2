use core::fmt::Debug;

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::Drawable;
#[cfg(feature = "cortex-m")]
use fugit::ExtU32;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::systick::Systick;

use super::Screen;
use crate::primitives::Cross;
use crate::{draw_badge, AppDrawTarget};

pub struct BootScreen<DT, E> {
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for BootScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        let x = (display.bounding_box().size.width / 2) as i32;
        let height = display.bounding_box().size.height;
        let y = (height / 2) as i32;

        Cross::new(Point::new(x, y + 10), 20, Rgb565::RED)
            .draw(display)
            .unwrap();
        draw_badge(
            display,
            Point::new(x, y),
            " ",
            Rgb565::CSS_GRAY,
            Rgb565::BLACK,
        )
        .await;
        draw_badge(
            display,
            Point::new(x, y),
            " XXX ",
            Rgb565::WHITE,
            Rgb565::BLACK,
        )
        .await;
        Cross::new(Point::new(x, y + 10), 30, Rgb565::WHITE)
            .draw(display)
            .unwrap();
        draw_badge(
            display,
            Point::new(x, y),
            env!("CARGO_PKG_VERSION"),
            Rgb565::BLACK,
            Rgb565::WHITE,
        )
        .await;
        #[cfg(feature = "cortex-m")]
        Systick::delay(150.millis()).await;
    }

    async fn draw_frame(&mut self, _display: &mut DT) {}
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for BootScreen<DT, E> {
    fn default() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}
