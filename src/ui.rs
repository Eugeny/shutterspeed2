use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use stm32f4xx_hal::pac::SPI1;
use stm32f4xx_hal::spi::Spi;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use u8g2_fonts::FontRenderer;
use ufmt::uwrite;

use crate::display::Display;
use crate::util::EString;

const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen16x32_me>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen12x24_me>();
const DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen32x64_mn>();

pub struct UiState<'a> {
    pub adc_value: u16,
    pub adc_history_iter: &'a mut dyn Iterator<Item = &'a u16>,
    pub counter: u32,
    pub sample_counter: u32,
}

pub fn init_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    TEXT_FONT
        .render(
            "Current value:",
            Point::new(50, 50),
            VerticalPosition::Top,
            // FontColor::Transparent( Rgb565::RED),
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();
}

pub fn draw_ui(display: &mut Display<Spi<SPI1>>, state: &mut UiState) {
    let mut s = EString::<128>::default();
    s.clear();
    let _ = uwrite!(s, "{}  ", state.adc_value);

    TEXT_FONT
        .render(
            ["*  ", " * ", "  *"][(state.counter % 3) as usize],
            Point::new(50, 20),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::WHITE,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();

    let res = DIGIT_FONT.render(
        &s[..],
        Point::new(50, 80),
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

    s.clear();
    let _ = uwrite!(s, "Samples: {}", state.sample_counter);
    SMALL_FONT
        .render(
            &s[..],
            Point::new(50, 180),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::WHITE,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();
    // let graph_rect = Rectangle::new(
    //     Point::new(0, display.height() as i32 - 20),
    //     Size::new(display.width(), 20),
    // );

    // display.fill_solid(&graph_rect, Rgb565::RED).unwrap();

    // for (i, adc_value) in state.adc_history_iter.enumerate() {
    //     let x = i as i32;
    //     let y = *adc_value as i32;

    //     let y = y * graph_rect.size.height as i32 / 4096;

    //     let x = x + graph_rect.top_left.x as i32;
    //     let y = graph_rect.bottom_right().unwrap().y as i32 - y;

    //     display
    //         .fill_solid(
    //             &Rectangle::new(Point::new(x, y), Size::new(2, 2)),
    //             Rgb565::BLACK,
    //         )
    //         .unwrap();
    // }
}
