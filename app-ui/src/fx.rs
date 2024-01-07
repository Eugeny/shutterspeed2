use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::{PointsIter, Rectangle};
use embedded_graphics::Pixel;

use crate::AppDrawTarget;

pub struct FX<'a, DT: AppDrawTarget<E>, E> {
    target: &'a mut DT,
    params: FXParams,
    _p: core::marker::PhantomData<E>,
}

#[derive(Copy, Clone)]
pub struct FXParams {
    t: u32,
}

impl Default for FXParams {
    fn default() -> Self {
        Self { t: 0 }
    }
}

impl<'a, DT: AppDrawTarget<E>, E> FX<'a, DT, E> {
    pub fn new(target: &'a mut DT, params: FXParams) -> Self {
        Self {
            target,
            params,
            _p: core::marker::PhantomData,
        }
    }

    pub fn inner(&mut self) -> &mut DT {
        self.target
    }

    fn map_pixel(mut p: Pixel<Rgb565>, params: FXParams) -> Rgb565 {
        let is_odd = (p.0.x % 2 == 1) ^ (p.0.y % 2 == 1) ^ (params.t % 2 == 1);
        const D: u16 =50;
        const DR: u8 = (D * Rgb565::MAX_R as u16 / 255) as u8;
        const DG: u8 = (D * Rgb565::MAX_G as u16 / 255) as u8;
        const DB: u8 = (D * Rgb565::MAX_B as u16 / 255) as u8;
        if is_odd {
            p.1 = Rgb565::new(
                p.1.r().saturating_sub(DR),
                p.1.g().saturating_sub(DG),
                p.1.b().saturating_sub(DB),
            );
        } else {
            p.1 = Rgb565::new(
                (p.1.r() + DR).min(Rgb565::MAX_R),
                (p.1.g() + DG).min(Rgb565::MAX_G),
                (p.1.b() + DB).min(Rgb565::MAX_B),
            );
        }
        p.1
    }

    pub fn step_params(&mut self) {
        self.params.step();
    }
}

impl FXParams {
    pub fn step(&mut self) {
        self.t += 1;
    }
}

impl<'a, E, DT: DrawTarget<Color = Rgb565, Error = E>> DrawTarget for FX<'a, DT, E> {
    type Color = Rgb565;
    type Error = E;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        let params = self.params;
        self.target.draw_iter(
            pixels
                .into_iter()
                .map(|p| Pixel(p.0, Self::map_pixel(p, params))),
        )
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        let params = self.params;
        self.target.fill_contiguous(
            area,
            area.points()
                .zip(colors)
                .map(|(pos, color)| Pixel(pos, color))
                .map(|p| Self::map_pixel(p, params)),
        )
    }
}

impl<'a, DT: AppDrawTarget<E>, E> Dimensions for FX<'a, DT, E> {
    fn bounding_box(&self) -> Rectangle {
        self.target.bounding_box()
    }
}
