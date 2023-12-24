use core::ops::{Deref, DerefMut};

use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::OriginDimensions;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_hal::blocking::delay::DelayUs;
use mipidsi::models::ST7789;
use stm32f4xx_hal::gpio::{ErasedPin, Output};

use crate::ui::draw_panic_screen;

pub struct Display<SPI: embedded_hal::blocking::spi::Write<u8>> {
    inner: mipidsi::Display<SPIInterfaceNoCS<SPI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>,
    backlight_pin: ErasedPin<Output>,
}

pub trait AppDrawTarget: DrawTarget<Color = Rgb565, Error = mipidsi::Error> {}

impl<D: DrawTarget<Color = Rgb565, Error = mipidsi::Error>> AppDrawTarget for D {}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> Display<SPI> {
    pub fn new<Delay: DelayUs<u32>>(
        spi: SPI,
        dc_pin: ErasedPin<Output>,
        rst_pin: ErasedPin<Output>,
        backlight_pin: ErasedPin<Output>,
        delay: &mut Delay,
    ) -> Self {
        let di = SPIInterfaceNoCS::new(spi, dc_pin);
        let display = match mipidsi::Builder::st7789(di)
            .with_orientation(mipidsi::Orientation::Landscape(true))
            .with_invert_colors(mipidsi::ColorInversion::Inverted)
            .init(delay, Some(rst_pin))
        {
            Ok(x) => x,
            Err(_) => panic!(),
        };
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

    pub fn panic_error<S: AsRef<str>>(&mut self, message: S) {
        draw_panic_screen(&mut self.inner, message.as_ref());
        panic!();
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> Deref for Display<SPI> {
    type Target =
        mipidsi::Display<SPIInterfaceNoCS<SPI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> DerefMut for Display<SPI> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
