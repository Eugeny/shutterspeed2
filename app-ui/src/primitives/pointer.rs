use core::ops::Mul;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::primitives::{
    Primitive, PrimitiveStyle, PrimitiveStyleBuilder, StrokeAlignment, Styled, Triangle,
};
use embedded_graphics::Drawable;

pub struct Pointer {
    triangle: Styled<Triangle, PrimitiveStyle<Rgb565>>,
}

impl Pointer {
    pub fn new(origin: Point, size: i32, upside_down: bool, color: Rgb565) -> Self {
        let style = PrimitiveStyleBuilder::new()
            .stroke_alignment(StrokeAlignment::Inside)
            .stroke_width(2)
            .stroke_color(color)
            .build();
        let sy = if upside_down { -1 } else { 1 };

        Self {
            triangle: Triangle::new(
                origin,
                origin - Point::new(-1, sy).mul(size),
                origin - Point::new(1, sy).mul(size),
            )
            .into_styled(style),
        }
    }
}

impl Drawable for Pointer {
    type Color = Rgb565;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.triangle.draw(target)?;
        Ok(())
    }
}
