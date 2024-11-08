use core::fmt::Debug;

use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;

use super::{DrawFrameContext, Screen};
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

    async fn draw_frame(&mut self, display: &mut DT, cx: DrawFrameContext) {
        let t = cx.animation_time_ms / 500;

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
