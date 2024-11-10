use core::fmt::{Debug, Write};

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::{Circle, PrimitiveStyle, StyledDrawable};
use heapless::String;

use super::{DrawFrameContext, Screen};
use crate::{draw_badge, AppDrawTarget};

pub struct CalibrationScreen<DT, E> {
    progress: u8,
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

    async fn draw_frame(&mut self, display: &mut DT, _cx: DrawFrameContext) {
        let mut s = String::<128>::default();
        write!(s, " {}%", self.progress).unwrap();

        let center = display.bounding_box().center();
        let sz = ((100 - self.progress) / 4) as u32;

        Circle::with_center(center, sz)
            .draw_styled(&PrimitiveStyle::with_stroke(Rgb565::YELLOW, 2), display)
            .unwrap();

        Circle::with_center(center, sz + 2)
            .draw_styled(&PrimitiveStyle::with_stroke(Rgb565::BLACK, 2), display)
            .unwrap();
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for CalibrationScreen<DT, E> {
    fn default() -> Self {
        Self {
            progress: 0,
            _phantom: core::marker::PhantomData,
        }
    }
}

impl<DT, E> CalibrationScreen<DT, E> {
    pub fn step(&mut self, progress: Option<u8>) {
        self.progress = progress.unwrap_or(100);
    }
}
