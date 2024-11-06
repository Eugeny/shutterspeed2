#[cfg(feature = "effects")]
use app_ui::FX;
use app_ui::{FXParams, HintRefresh};
use config as hw;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Dimensions;
use embedded_graphics::pixelcolor::Rgb565;
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Pixel;
use hw::display_interface_spi::SPIInterface;
use hw::hal::gpio::{ErasedPin, Output};
use mipidsi::models::ST7735s;

pub trait DisplayInterface: embedded_hal::spi::SpiDevice<u8> {}
impl<W: embedded_hal::spi::SpiDevice<u8>> DisplayInterface for W {}

pub struct Display<DI: DisplayInterface> {
    inner: mipidsi::Display<SPIInterface<DI, ErasedPin<Output>>, ST7735s, ErasedPin<Output>>,
    backlight_pin: ErasedPin<Output>,
    fx_params: FXParams,
}

impl<DI: DisplayInterface> Display<DI> {
    pub fn new(
        inner: mipidsi::Display<
            SPIInterface<DI, ErasedPin<Output>>,
            ST7735s,
            ErasedPin<Output>,
        >,
        backlight_pin: ErasedPin<Output>,
    ) -> Self {
        Display {
            inner,
            backlight_pin,
            fx_params: FXParams::default(),
        }
    }

    pub fn step_fx(&mut self) {
        self.fx_params.step();
    }

    pub fn backlight_on(&mut self) {
        self.backlight_pin.set_high();
    }

    pub fn backlight_off(&mut self) {
        self.backlight_pin.set_low();
    }

    pub fn sneaky_clear(&mut self, color: Rgb565) {
        self.backlight_off();
        self.inner.clear(color).unwrap();
        self.backlight_on();
    }

    pub fn height(&self) -> u32 {
        self.bounding_box().size.height
    }

    pub fn width(&self) -> u32 {
        self.bounding_box().size.width
    }
}

impl<DI: DisplayInterface> Dimensions for Display<DI> {
    fn bounding_box(&self) -> Rectangle {
        self.inner.bounding_box()
    }
}

impl<DI: DisplayInterface> HintRefresh for Display<DI> {
    fn hint_refresh(&mut self) {}
}

impl<DI: DisplayInterface> DrawTarget for Display<DI> {
    type Color = Rgb565;
    type Error = mipidsi::error::Error;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        #[cfg(feature = "effects")]
        let mut d = FX::new(&mut self.inner, self.fx_params);
        #[cfg(not(feature = "effects"))]
        let d = &mut self.inner;
        d.draw_iter(pixels)
    }

    fn fill_contiguous<I>(&mut self, area: &Rectangle, colors: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Color>,
    {
        #[cfg(feature = "effects")]
        let mut d = FX::new(&mut self.inner, self.fx_params);
        #[cfg(not(feature = "effects"))]
        let d = &mut self.inner;
        d.fill_contiguous(area, colors)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.inner.fill_solid(&self.bounding_box(), color)
    }
}
