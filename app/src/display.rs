use core::ops::{Deref, DerefMut};

use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_hal::blocking::delay::DelayUs;
use mipidsi::models::ST7789;
use stm32f4xx_hal::gpio::{ErasedPin, Output};

pub trait DisplayInterface: embedded_hal::blocking::spi::Write<u8> {}
impl<W: embedded_hal::blocking::spi::Write<u8>> DisplayInterface for W {}

pub trait AppDrawTarget: DrawTarget<Color = Rgb565, Error = mipidsi::Error> {}
impl<D: DrawTarget<Color = Rgb565, Error = mipidsi::Error>> AppDrawTarget for D {}

pub struct Display<DI: DisplayInterface> {
    inner: mipidsi::Display<SPIInterfaceNoCS<DI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>,
    backlight_pin: ErasedPin<Output>,
}

impl<DI: DisplayInterface> Display<DI> {
    pub fn new<Delay: DelayUs<u32>>(
        spi: DI,
        dc_pin: ErasedPin<Output>,
        rst_pin: ErasedPin<Output>,
        backlight_pin: ErasedPin<Output>,
        delay: &mut Delay,
    ) -> Self {
        let di = SPIInterfaceNoCS::new(spi, dc_pin);
        let display = mipidsi::Builder::st7789(di)
            .with_orientation(mipidsi::Orientation::Portrait(false))
            .with_invert_colors(mipidsi::ColorInversion::Inverted)
            .init(delay, Some(rst_pin))
            .unwrap();
        Display {
            inner: display,
            backlight_pin,
        }
    }

    pub fn clear(&mut self) {
        self.inner.clear(Rgb565::BLACK).unwrap();
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
        self.size().height
    }

    pub fn width(&self) -> u32 {
        self.size().width
    }
}

impl<DI: DisplayInterface> Deref for Display<DI> {
    type Target =
        mipidsi::Display<SPIInterfaceNoCS<DI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<DI: DisplayInterface> DerefMut for Display<DI> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
