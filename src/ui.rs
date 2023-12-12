use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use stm32f4xx_hal::pac::SPI1;
use stm32f4xx_hal::spi::Spi;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::FontRenderer;
use ufmt::uwrite;

use crate::display::Display;
use crate::util::EString;

const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen16x32_me>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_profont29_mr>();
const DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen32x64_mn>();

pub struct ResultsUiState {
    pub calibration_value: u16,
}

pub struct DebugUiState {
    pub adc_value: u16,
    // pub adc_history_iter: &'a mut dyn Iterator<Item = &'a u16>,
    pub sample_counter: u32,
}

pub fn init_start_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    SMALL_FONT
        .render_aligned(
            " PUSH TO MEASURE ",
            Point::new(display.width() as i32 / 2, 100),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground {
                fg: Rgb565::BLUE,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();
}

pub fn draw_start_ui(_display: &mut Display<Spi<SPI1>>) {}

pub fn init_calibrating_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    SMALL_FONT
        .render_aligned(
            " CALIBRATING ",
            Point::new(display.width() as i32 / 2, 100),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground {
                fg: Rgb565::BLACK,
                bg: Rgb565::YELLOW,
            },
            &mut **display,
        )
        .unwrap();
}

pub fn init_measuring_ui(display: &mut Display<Spi<SPI1>>) {
    // display.clear();

    SMALL_FONT
        .render_aligned(
            "  MEASURING  ",
            Point::new(display.width() as i32 / 2, 100),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground {
                fg: Rgb565::BLACK,
                bg: Rgb565::RED,
            },
            &mut **display,
        )
        .unwrap();
}

pub fn draw_measuring_ui(_display: &mut Display<Spi<SPI1>>) {}

pub fn init_results_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    TEXT_FONT
        .render(
            "Results",
            Point::new(50, 10),
            VerticalPosition::Top,
            // FontColor::Transparent( Rgb565::RED),
            FontColor::WithBackground {
                fg: Rgb565::GREEN,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();

    SMALL_FONT
        .render(
            "Calibrated to:",
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

pub fn draw_results_ui(display: &mut Display<Spi<SPI1>>, state: &ResultsUiState) {
    let mut s = EString::<128>::default();
    s.clear();
    let _ = uwrite!(s, "{}  ", state.calibration_value);
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
}

pub fn init_debug_ui(display: &mut Display<Spi<SPI1>>) {
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

pub fn draw_debug_ui(display: &mut Display<Spi<SPI1>>, state: &mut DebugUiState) {
    let mut s = EString::<128>::default();
    s.clear();
    let _ = uwrite!(s, "{}  ", state.adc_value);
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
