use core::ops::{Deref, DerefMut};

use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Point, Size};
use embedded_graphics::mono_font::ascii::FONT_10X20;
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use embedded_hal::blocking::delay::DelayUs;
use embedded_text::alignment::HorizontalAlignment;
use embedded_text::style::{HeightMode, TextBoxStyleBuilder};
use embedded_text::TextBox;
use mipidsi::models::ST7789;
use stm32f4xx_hal::gpio::{ErasedPin, Output};

pub struct Display<SPI: embedded_hal::blocking::spi::Write<u8>> {
    inner: mipidsi::Display<SPIInterfaceNoCS<SPI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>,
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> Display<SPI> {
    pub fn new<Delay: DelayUs<u32>>(
        spi: SPI,
        dc_pin: ErasedPin<Output>,
        rst_pin: ErasedPin<Output>,
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
        Display { inner: display }
    }

    pub fn clear(&mut self) {
        self.inner.clear(Rgb565::BLACK).unwrap();
    }

    pub fn height(&self) -> u32 {
        self.size().height
    }

    pub fn width(&self) -> u32 {
        self.size().width
    }

    pub fn panic_error<S: AsRef<str>>(&mut self, message: S) {
        self.clear();

        let character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);

        let textbox_style = TextBoxStyleBuilder::new()
            .height_mode(HeightMode::FitToText)
            .alignment(HorizontalAlignment::Center)
            .build();

        let _ = TextBox::with_textbox_style(
            "FATAL ERROR",
            Rectangle::new(Point::new(10, 20), Size::new(self.width() - 20, 100)),
            character_style,
            textbox_style,
        )
        .draw(&mut self.inner);
        let _ = TextBox::with_textbox_style(
            message.as_ref(),
            Rectangle::new(
                Point::new(10, 60),
                Size::new(self.width() - 20, self.height() - 20),
            ),
            character_style,
            textbox_style,
        )
        .draw(&mut self.inner);

        panic!();
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> Deref
    for Display<SPI>
{
    type Target = mipidsi::Display<SPIInterfaceNoCS<SPI, ErasedPin<Output>>, ST7789, ErasedPin<Output>>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>> DerefMut
    for Display<SPI>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
