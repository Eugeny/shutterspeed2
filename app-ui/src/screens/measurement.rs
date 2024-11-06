use core::fmt::Debug;

use embedded_graphics::geometry::{Dimensions, Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::systick::Systick;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::Monotonic;

use super::{DrawFrameContext, Screen};
use crate::{draw_badge, AppDrawTarget};

pub struct MeasurementScreen<DT, E> {
    _phantom: core::marker::PhantomData<(DT, E)>,
}

fn progress_origin<D: Dimensions>(d: &D) -> Point {
    d.bounding_box().center() + Point::new(0, 10)
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for MeasurementScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_badge(
            display,
            display.bounding_box().center() - Point::new(0, 30),
            "  MEASURING  ",
            Rgb565::BLACK,
            Rgb565::RED,
        )
        .await;

        display
            .fill_solid(
                &Rectangle::with_center(progress_origin(display), Size::new(40, 11)),
                Rgb565::RED,
            )
            .unwrap();
    }

    async fn draw_frame(&mut self, display: &mut DT, cx: DrawFrameContext) {
        let t = cx.animation_time_ms / 1000;

        let offsets = -1i32..2;
        let len = offsets.len() as u32;
        let origin = progress_origin(display);
        for (idx, dx) in offsets.enumerate() {
            let color = if idx as u32 == t % len {
                Rgb565::RED
            } else {
                Rgb565::BLACK
            };
            display
                .fill_solid(
                    &Rectangle::with_center(origin + Point::new(dx * 10, 0), Size::new(5, 5)),
                    color,
                )
                .unwrap();
        }
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for MeasurementScreen<DT, E> {
    fn default() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}
