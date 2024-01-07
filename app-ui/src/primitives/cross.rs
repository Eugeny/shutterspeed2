use core::ops::Mul;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::primitives::{Line, Primitive, PrimitiveStyle, Styled};
use embedded_graphics::Drawable;

pub struct Cross {
    line1: Styled<Line, PrimitiveStyle<Rgb565>>,
    line2: Styled<Line, PrimitiveStyle<Rgb565>>,
}

impl Cross {
    pub fn new(origin: Point, size: i32, color: Rgb565) -> Self {
        let style = PrimitiveStyle::with_stroke(color, 4);

        Self {
            line1: Line::new(
                origin + Point::new(-1, -1).mul(size),
                origin + Point::new(1, 1).mul(size),
            )
            .into_styled(style),
            line2: Line::new(
                origin + Point::new(1, -1).mul(size),
                origin + Point::new(-1, 1).mul(size),
            )
            .into_styled(style),
        }
    }
}

impl Drawable for Cross {
    type Color = Rgb565;
    type Output = ();

    fn draw<D>(&self, target: &mut D) -> Result<Self::Output, D::Error>
    where
        D: DrawTarget<Color = Self::Color>,
    {
        self.line1.draw(target)?;
        self.line2.draw(target)?;
        Ok(())
    }
}
