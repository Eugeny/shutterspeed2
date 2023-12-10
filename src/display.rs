use core::ops::{Deref, DerefMut};

use display_interface_spi::SPIInterfaceNoCS;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::Point;
use embedded_graphics::mono_font::ascii::{FONT_5X7, FONT_10X20};
use embedded_graphics::mono_font::MonoTextStyle;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::text::renderer::CharacterStyle;
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use embedded_hal::blocking::delay::DelayUs;
use embedded_hal::digital::v2::OutputPin;
use mipidsi::models::ST7789;

pub struct Display<SPI: embedded_hal::blocking::spi::Write<u8>, DC: OutputPin, RST: OutputPin> {
    inner: mipidsi::Display<SPIInterfaceNoCS<SPI, DC>, ST7789, RST>,
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>, DC: OutputPin, RST: OutputPin>
    Display<SPI, DC, RST>
{
    pub fn new<Delay: DelayUs<u32>>(spi: SPI, dc_pin: DC, rst_pin: RST, delay: &mut Delay) -> Self {
        let di = SPIInterfaceNoCS::new(spi, dc_pin);

        let mut display = match mipidsi::Builder::st7789(di)
            .with_orientation(mipidsi::Orientation::Landscape(true))
            .with_invert_colors(mipidsi::ColorInversion::Inverted)
            .init(delay, Some(rst_pin))
        {
            Ok(x) => x,
            Err(_) => {
                // show_error!(lcd, "mipidsi builder");
                loop {}
            }
        };
        Display { inner: display }
    }

    pub fn clear(&mut self) {
        self.inner.clear(Rgb565::BLACK).unwrap();
    }

    pub fn panic_error<S: AsRef<str>>(&mut self, message: S) {
        self.clear();
        let mut character_style = MonoTextStyle::new(&FONT_10X20, Rgb565::RED);
        character_style.set_background_color(Some(Rgb565::BLACK));

        let _ = Text::new("ERROR", Point::new(10, 10), character_style).draw(&mut self.inner);
        let _ =
            Text::new(message.as_ref(), Point::new(10, 25), character_style).draw(&mut self.inner);
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>, DC: OutputPin, RST: OutputPin> Deref
    for Display<SPI, DC, RST>
{
    type Target = mipidsi::Display<SPIInterfaceNoCS<SPI, DC>, ST7789, RST>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<SPI: embedded_hal::blocking::spi::Write<u8>, DC: OutputPin, RST: OutputPin> DerefMut
    for Display<SPI, DC, RST>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}
