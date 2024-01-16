use core::fmt::Debug;

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::Drawable;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use super::Screen;
use crate::fonts::{SMALL_FONT, TINY_FONT};
use crate::primitives::Cross;
use crate::AppDrawTarget;

pub struct UpdateScreen<DT, E> {
    _phantom: core::marker::PhantomData<(DT, E)>,
}

const COLOR: Rgb565 = Rgb565::CSS_GRAY;

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for UpdateScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        let width = display.bounding_box().size.width;

        display.fill_solid(&display.bounding_box(), COLOR).unwrap();

        for d in [-1, 0, 1] {
            let _ = Cross::new(Point::new(width as i32 / 2 + d * 20, 25), 7, Rgb565::BLACK)
                .draw(display);
        }

        TINY_FONT
            .render_aligned(
                env!("CARGO_PKG_VERSION"),
                Point::new(width as i32 / 2, 45),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: Rgb565::BLACK,
                    bg: COLOR,
                },
                display,
            )
            .unwrap();

        SMALL_FONT
            .render_aligned(
                " REBOOTING ",
                Point::new(width as i32 / 2, 60),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: COLOR,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();
    }

    async fn draw_frame(&mut self, _display: &mut DT) {}
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for UpdateScreen<DT, E> {
    fn default() -> Self {
        Self {
            _phantom: core::marker::PhantomData,
        }
    }
}
