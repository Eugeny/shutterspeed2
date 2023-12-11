use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use stm32f4xx_hal::pac::SPI1;
use stm32f4xx_hal::spi::Spi;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use u8g2_fonts::FontRenderer;
use ufmt::uwrite;

use crate::display::Display;
use crate::util::EString;

const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen16x32_me>();
const DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen32x64_mn>();

pub struct UiState {
    pub adc_value: u16,
}

pub fn draw_ui(display: &mut Display<Spi<SPI1>>, state: &UiState) {
    let mut s = EString::<128>::default();
    let _ = uwrite!(s, "{}  ", state.adc_value);

    let res = TEXT_FONT.render(
        "Current value:",
        Point::new(50, 50),
        VerticalPosition::Top,
        // FontColor::Transparent( Rgb565::RED),
        FontColor::WithBackground {
            fg: Rgb565::RED,
            bg: Rgb565::BLACK,
        },
        &mut **display,
    );
    if let Err(err) = res {
        s.clear();
        use core::fmt::Write;
        let _ = write!(*s, "Failed with: {:?}", err);
        display.panic_error(&s[..]);
    }

    let res = DIGIT_FONT.render(
        &s[..],
        Point::new(50, 100),
        VerticalPosition::Top,
        FontColor::WithBackground {
            fg: Rgb565::RED,
            bg: Rgb565::BLACK,
        },
        &mut **display,
    );
    if let Err(err) = res {
        s.clear();
        use core::fmt::Write;
        let _ = write!(*s, "Failed with: {:?}", err);
        display.panic_error(&s[..]);
    }
}
