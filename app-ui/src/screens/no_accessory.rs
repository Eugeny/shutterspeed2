use core::fmt::Debug;

use embedded_graphics::draw_target::DrawTargetExt;
use embedded_graphics::geometry::Point;
use embedded_graphics::image::Image;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::prelude::Dimensions;
use embedded_graphics::primitives::{Polyline, PrimitiveStyle, StyledDrawable};
use embedded_graphics::Drawable;
use tinybmp::Bmp;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use super::{DrawFrameContext, Screen};
use crate::fonts::TINY_FONT;
use crate::{draw_badge, AppDrawTarget};

pub struct NoAccessoryScreen<DT, E> {
    img: Bmp<'static, Rgb565>,
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for NoAccessoryScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_badge(
            display,
            display.bounding_box().center() - Point::new(0, 40),
            " NO SENSOR ",
            Rgb565::BLACK,
            Rgb565::RED,
        )
        .await;

        TINY_FONT
            .render_aligned(
                " ATTACH A MODULE ",
                display.bounding_box().center() - Point::new(0, 20),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: Rgb565::RED,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        Image::new(
            &self.img,
            display.bounding_box().center() - self.img.bounding_box().size / 2 + Point::new(0, 50),
        )
        .draw(display)
        .unwrap();
    }

    async fn draw_frame(&mut self, display: &mut DT, cx: DrawFrameContext) {
        let t = cx.animation_time_ms / 150;

        let offsets = [12, 12, 12, 9, 6, 4, 2, 2, 2, 2, 2];
        let current_index = t as usize % offsets.len();

        for (i, offset) in offsets.iter().enumerate() {
            let center = Point::new((display.bounding_box().size.width / 2) as i32, 5 + offset);

            let points = [
                center,
                center + Point::new(-10, 5),
                center,
                center + Point::new(10, 5),
                center,
            ];
            let t = Polyline::new(&points);

            for dy in [0, 1] {
                t.draw_styled(
                    &PrimitiveStyle::with_stroke(
                        if offsets[i] == offsets[current_index] {
                            Rgb565::WHITE
                        } else {
                            Rgb565::BLACK
                        },
                        1,
                    ),
                    &mut display.translated(Point::new(0, dy)),
                )
                .unwrap();
            }
        }
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for NoAccessoryScreen<DT, E> {
    fn default() -> Self {
        let bmp_data = include_bytes!("../../images/goober.bmp");
        let img = Bmp::from_slice(bmp_data).unwrap();
        Self {
            img,
            _phantom: core::marker::PhantomData,
        }
    }
}
