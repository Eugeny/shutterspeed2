use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Pixel;
use stm32f4xx_hal::pac::SPI1;
use stm32f4xx_hal::spi::Spi;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::FontRenderer;
use ufmt::{uWrite, uwrite};

use crate::display::Display;
use crate::measurement::{CalibrationState, MeasurementResult};
use crate::util::EString;

const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen16x32_me>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_profont29_mr>();
const DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_fonts::fonts::u8g2_font_spleen32x64_mn>();

pub struct ResultsUiState {
    pub calibration: CalibrationState,
    pub result: MeasurementResult,
    pub result_samples: u32,
}

pub struct DebugUiState {
    pub adc_value: u16,
    pub min_adc_value: u16,
    pub max_adc_value: u16,
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
                bg: Rgb565::GREEN,
                fg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();

    // SMALL_FONT
    //     .render(
    //         "Calibrated to:",
    //         Point::new(50, 50),
    //         VerticalPosition::Top,
    //         // FontColor::Transparent( Rgb565::RED),
    //         FontColor::WithBackground {
    //             fg: Rgb565::RED,
    //             bg: Rgb565::BLACK,
    //         },
    //         &mut **display,
    //     )
    //     .unwrap();
}

pub fn draw_results_ui(display: &mut Display<Spi<SPI1>>, state: &ResultsUiState) {
    if let CalibrationState::Done(ref value) = state.calibration {
        let mut s = EString::<128>::default();
        s.clear();
        let _ = uwrite!(s, "Calibrated: {}", value);
        let _ = SMALL_FONT.render(
            &s[..],
            Point::new(20, 215),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        );
    }

    {
        let mut s = EString::<128>::default();
        s.clear();
        if state.result.duration_micros < 500_000 {
            let _ = uwrite!(s, "1/");
            _write_fraction(
                &mut s,
                1_000_000 as f32 / state.result.duration_micros as f32,
            );
        } else {
            let _ = _write_fraction(
                &mut s,
                state.result.duration_micros as f32 / 1_000_000 as f32,
            );
        }
        let _ = uwrite!(s, " s");
        let _ = SMALL_FONT.render(
            &s[..],
            Point::new(20, 185),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        );
    }

    {
        let mut s = EString::<128>::default();
        s.clear();
        let _ = uwrite!(s, "~ {} us", state.result.duration_micros);
        let _ = SMALL_FONT.render(
            &s[..],
            Point::new(20, 155),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        );
    }

    {
        let mut s = EString::<128>::default();
        s.clear();
        let _ = uwrite!(s, "Duration: {} smp", state.result_samples);
        let _ = SMALL_FONT.render(
            &s[..],
            Point::new(20, 125),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        );
    }

    draw_chart(display, &state.result, 40);
}

fn draw_chart(display: &mut Display<Spi<SPI1>>, result: &MeasurementResult, graph_y: i32) {
    let chart = &result.fall_buffer;
    let len = chart.len();
    let width = display.width();
    let graph_rect = Rectangle::new(Point::new(0, graph_y), Size::new(width, 40));

    let min = chart.iter().min().cloned().unwrap_or(0);
    let max = chart.iter().max().cloned().unwrap_or(0).max(min + 1);

    let chunk_size = (len / width as usize).max(1);
    let mut i = 0;
    let mut done = false;
    let mut iter = chart.oldest_ordered();

    let xy_to_coords = |x: u16, y: u16| {
        let x = x / chunk_size as u16;
        let y = (y - min) as i32;

        let y = y * graph_rect.size.height as i32 / (max - min) as i32;

        let x = x as i32 + graph_rect.top_left.x;
        let y = graph_rect.bottom_right().unwrap().y as i32 - y;
        (x, y)
    };

    while !done {
        let mut sum = 0;
        let mut count = 0;
        for _ in 0..chunk_size {
            if let Some(x) = iter.next() {
                sum += x;
                count += 1;
            } else {
                done = true;
                break;
            }
        }
        if count == 0 {
            break;
        }
        let avg: u16 = sum / count;

        let (x, y) = xy_to_coords(i * chunk_size as u16, avg);
        display
            .fill_solid(
                &Rectangle::new(Point::new(x, y), Size::new(2, 2)),
                Rgb565::YELLOW,
            )
            .unwrap();

        i += 1;
    }

    let start_x = chart.len() - result.samples_since_start as usize;
    if let Some(start_y) = chart.get(start_x) {
        let (x, y) = xy_to_coords(start_x as u16, *start_y);
        draw_cross(display, Point::new(x, y), 5, Rgb565::GREEN);
    }

    let end_x = chart.len() - result.samples_since_end as usize;
    if let Some(end_y) = chart.get(end_x) {
        let (x, y) = xy_to_coords(end_x as u16, *end_y);
        draw_cross(display, Point::new(x, y), 5, Rgb565::RED);
    }
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

    let variation = (state.max_adc_value - state.min_adc_value) / 2;

    let _ = uwrite!(s, "{} +-{}  ", state.adc_value, variation);
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

fn _write_fraction<W: uWrite>(s: &mut W, fraction: f32) {
    let int = fraction as u32;
    let fr = (fraction - int as f32) * 10.0;
    let _ = uwrite!(s, "{}.{}", int, fr as u32);
}

pub fn draw_cross(display: &mut Display<Spi<SPI1>>, point: Point, size: u32, color: Rgb565) {
    for dir in [-1, 1] {
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            display
                .draw_iter(
                    (-(size as i32)..size as i32).into_iter().map(|i| {
                        Pixel(Point::new(point.x + dx + i, point.y + dy + i * dir), color)
                    }),
                )
                .unwrap();
        }
    }
}
