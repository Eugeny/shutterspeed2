use core::ops::DerefMut;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Dimensions, Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::{Drawable, Pixel};
use embedded_text::style::{HeightMode, TextBoxStyleBuilder};
use embedded_text::TextBox;
use hal::pac::SPI1;
use hal::prelude::*;
use hal::spi::Spi;
use heapless::{HistoryBuffer, String};
use micromath::F32Ext;
use rtic_monotonics::systick::Systick;
use stm32f4xx_hal as hal;
use u8g2_fonts::fonts::{u8g2_font_profont17_mr, u8g2_font_profont29_mr, u8g2_font_spleen32x64_mn};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{FontRenderer, U8g2TextStyle};
use ufmt::uwrite;

use crate::display::{AppDrawTarget, Display};
use crate::format::{write_fraction, write_micros};
use crate::measurement::{CalibrationState, MeasurementResult};
use crate::util::LaxMonotonic;

// const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen16x32_me>();
const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont29_mr>();
const TINY_FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont17_mr>();
const LARGE_DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen32x64_mn>();

pub struct ResultsUiState {
    pub calibration: CalibrationState,
    pub result: MeasurementResult,
}

pub struct DebugUiState<'a> {
    pub adc_value: u16,
    pub min_adc_value: u16,
    pub max_adc_value: u16,
    pub adc_history: &'a HistoryBuffer<u16, 100>,
    pub sample_counter: u32,
}

pub async fn init_start_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    draw_badge(
        display,
        Point::new(display.width() as i32 / 2, 100),
        " READY ",
        Rgb565::CSS_PALE_GREEN,
        Rgb565::BLACK,
    )
    .await;
}

pub fn draw_start_ui(display: &mut Display<Spi<SPI1>>) {
    let t = (Systick::now() - <Systick as rtic_monotonics::Monotonic>::ZERO).to_millis() / 500;
    let color = if t % 2 == 0 {
        Rgb565::WHITE
    } else {
        Rgb565::BLACK
    };
    let center = display.bounding_box().center();
    display
        .fill_solid(
            &Rectangle::with_center(center + Point::new(0, 40), Size::new(10, 10)),
            color,
        )
        .unwrap();
}

pub async fn init_calibrating_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    draw_badge(
        display,
        Point::new(display.width() as i32 / 2, 100),
        " CALIBRATING ",
        Rgb565::BLACK,
        Rgb565::YELLOW,
    )
    .await;
}

pub async fn init_measuring_ui(display: &mut Display<Spi<SPI1>>) {
    draw_badge(
        display,
        Point::new(display.width() as i32 / 2, 100),
        "  MEASURING  ",
        Rgb565::BLACK,
        Rgb565::RED,
    )
    .await;
}

pub fn draw_measuring_ui(display: &mut Display<Spi<SPI1>>) {
    let t = (Systick::now() - <Systick as rtic_monotonics::Monotonic>::ZERO).to_secs();
    let offsets = -2i32..2;
    let len = offsets.len() as u32;
    for (idx, dx) in offsets.enumerate() {
        let x = display.width() as i32 / 2 + dx * 10;
        let y = 150;
        let color = if idx as u32 == t % len {
            Rgb565::RED
        } else {
            Rgb565::BLACK
        };
        display
            .fill_solid(
                &Rectangle::with_center(Point::new(x, y), Size::new(5, 5)),
                color,
            )
            .unwrap();
    }
}

pub async fn init_results_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    // draw_badge(
    //     display,
    //     Point::new(display.width() as i32 / 2, 5),
    //     " RESULTS ",
    //     Rgb565::GREEN,
    //     Rgb565::BLACK,
    // )
    // .await
}

pub fn draw_results_ui(display: &mut Display<Spi<SPI1>>, state: &ResultsUiState) {
    let exposure_time_origin = Point::new(20, 100);
    {
        let duration_micros = state.result.integrated_duration_micros.max(1);
        let mut s = String::<128>::default();

        let dim = if duration_micros < 500_000 {
            write_fraction(&mut s, 1_000_000_f32 / duration_micros as f32);
            SMALL_FONT
                .render(
                    "1/",
                    exposure_time_origin + Point::new(0, 30),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::CSS_LIGHT_GRAY,
                        bg: Rgb565::BLACK,
                    },
                    &mut **display,
                )
                .unwrap();
            LARGE_DIGIT_FONT
                .render(
                    &s[..],
                    exposure_time_origin + Point::new(30, 15),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::WHITE,
                        bg: Rgb565::BLACK,
                    },
                    &mut **display,
                )
                .unwrap()
        } else {
            write_fraction(&mut s, duration_micros as f32 / 1_000_000_f32);
            LARGE_DIGIT_FONT
                .render(
                    &s[..],
                    exposure_time_origin + Point::new(0, 15),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::WHITE,
                        bg: Rgb565::BLACK,
                    },
                    &mut **display,
                )
                .unwrap()
        };
        SMALL_FONT
            .render(
                "s",
                dim.bounding_box.unwrap().bottom_right().unwrap() + Point::new(5, -27),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: Rgb565::CSS_LIGHT_GRAY,
                    bg: Rgb565::BLACK,
                },
                &mut **display,
            )
            .unwrap();

        TINY_FONT
            .render(
                " Exposure time ",
                exposure_time_origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: Rgb565::WHITE,
                    fg: Rgb565::BLACK,
                },
                &mut **display,
            )
            .unwrap();
    }

    {
        let mut s = String::<128>::default();
        s.push(' ').unwrap();
        write_micros(&mut s, state.result.integrated_duration_micros);
        let _ = s.push(' ');
        TINY_FONT
            .render(
                &s[..],
                exposure_time_origin + Point::new(150, 0),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: Rgb565::CSS_ORANGE_RED,
                    fg: Rgb565::BLACK,
                },
                &mut **display,
            )
            .unwrap();
    }

    // {
    //     let mut s = String::<128>::default();
    //     s.clear();
    //     uwrite!(s, "{} us start to end", state.result.duration_micros,).unwrap();
    //     TINY_FONT.render(
    //         &s[..],
    //         Point::new(20, 195),
    //         VerticalPosition::Top,
    //         FontColor::WithBackground {
    //             fg: Rgb565::RED,
    //             bg: Rgb565::BLACK,
    //         },
    //         &mut **display,
    //     ).unwrap();
    // }
    // {
    // let mut s = String::<128>::default();
    // s.clear();
    // uwrite!(
    //     s,
    //     "Captured {} samples",
    //     state.result.sample_buffer.len(),
    //     // state.result.samples_since_end - state.result.samples_since_start
    // ).unwrap();
    // TINY_FONT.render(
    //     &s[..],
    //     Point::new(20, 215),
    //     VerticalPosition::Top,
    //     FontColor::WithBackground {
    //         fg: Rgb565::RED,
    //         bg: Rgb565::BLACK,
    //     },
    //     &mut **display,
    // ).unwrap();
    // }

    draw_speed_ruler(
        display,
        Point::new(0, 215),
        state.result.integrated_duration_micros as f32 / 1_000_000.0,
    );

    draw_chart(
        display,
        &state.result.sample_buffer,
        25,
        Some(state.result.samples_since_start),
        Some(state.result.samples_since_end),
        false,
    );
}

fn draw_speed_ruler(display: &mut Display<Spi<SPI1>>, origin: Point, actual_duration_secs: f32) {
    let width = display.width();
    let ruler_height = 10;

    let duration_to_x_offset = |d: f32| ((1.0 / d).log2() * 60.0) as i32;

    let actual_x = origin.x + duration_to_x_offset(actual_duration_secs);

    let overall_x_offset = display.width() as i32 / 2 - actual_x;

    let known_durations = [
        8.0,
        4.0,
        2.0,
        1.0,
        1.0 / 2.0,
        1.0 / 4.0,
        1.0 / 8.0,
        1.0 / 15.0,
        1.0 / 30.0,
        1.0 / 60.0,
        1.0 / 125.0,
        1.0 / 250.0,
        1.0 / 500.0,
        1.0 / 1000.0,
        1.0 / 2000.0,
        1.0 / 4000.0,
        1.0 / 8000.0,
        1.0 / 16000.0,
    ];

    display
        .fill_contiguous(
            &Rectangle::new(
                origin - Point::new(0, ruler_height),
                Size::new(width - 1, ruler_height as u32),
            ),
            [Rgb565::CSS_DARK_GREEN, Rgb565::BLACK]
                .iter()
                .cycle()
                .cloned(),
        )
        .unwrap();
    display
        .fill_solid(
            &Rectangle::new(origin, Size::new(width, 1)),
            Rgb565::CSS_PALE_GREEN,
        )
        .unwrap();
    display
        .fill_solid(
            &Rectangle::new(origin + Point::new(0, -ruler_height), Size::new(width, 1)),
            Rgb565::CSS_PALE_GREEN,
        )
        .unwrap();

    let mut best_match = 1.0;
    for duration in known_durations.iter() {
        if (duration.log2() - actual_duration_secs.log2()).abs()
            < (best_match.log2() - actual_duration_secs.log2()).abs()
        {
            best_match = *duration;
        }
    }

    for duration in known_durations.iter() {
        let x = origin.x + overall_x_offset + duration_to_x_offset(*duration);
        let y = origin.y;
        let mut s = String::<128>::default();
        s.clear();
        let mut color = if duration >= &1.0 {
            uwrite!(s, " {} ", duration.round() as u32).unwrap();
            Rgb565::CSS_ORANGE
        } else {
            uwrite!(s, " {} ", (1.0 / duration).round() as u32).unwrap();
            Rgb565::CSS_PALE_GREEN
        };
        if best_match == *duration {
            color = Rgb565::MAGENTA;
            draw_triangle(
                display,
                Point::new(x, y - ruler_height - 1),
                10,
                false,
                color,
            );
        }

        let label_size = TINY_FONT
            .get_rendered_dimensions(&s[..], Point::zero(), VerticalPosition::Top)
            .unwrap();
        let label_origin = Point::new(
            x - label_size.bounding_box.unwrap().size.width as i32 / 2,
            y + 7,
        );

        let label_off_screen = label_origin.x + label_size.bounding_box.unwrap().size.width as i32
            > width as i32
            || label_origin.x < 0;

        display
            .fill_solid(
                &Rectangle::new(
                    Point::new(x - 1, y - ruler_height),
                    Size::new(
                        3,
                        ruler_height as u32 + if label_off_screen { 0 } else { 5 },
                    ),
                ),
                color,
            )
            .unwrap();

        if label_off_screen {
            continue;
        }
        TINY_FONT
            .render(
                &s[..],
                label_origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: color,
                    fg: Rgb565::BLACK,
                },
                &mut **display,
            )
            .unwrap();
    }

    draw_triangle(
        display,
        Point::new(overall_x_offset + actual_x - 2, origin.y - ruler_height - 1),
        12,
        false,
        Rgb565::WHITE,
    );
}

fn draw_chart<const LEN: usize>(
    display: &mut Display<Spi<SPI1>>,
    chart: &HistoryBuffer<u16, LEN>,
    graph_y: i32,
    samples_since_start: Option<usize>,
    samples_since_end: Option<usize>,
    clear: bool,
) {
    let padding = 10;

    let len = chart.len();
    let width = display.width() - padding * 2;
    let graph_rect = Rectangle::new(Point::new(padding as i32, graph_y), Size::new(width, 40));

    if clear {
        display.fill_solid(&graph_rect, Rgb565::BLACK).unwrap();
    }

    let min = chart.iter().min().cloned().unwrap_or(0);
    let max = chart.iter().max().cloned().unwrap_or(0).max(min + 1);

    let chunk_size = ((len as f32 / width as f32).ceil() as u32).max(1);
    let mut i = 0;
    let mut done = false;
    let mut iter = chart.oldest_ordered();

    let xy_to_coords = |x: u16, y: u16| {
        let x = x / chunk_size as u16;
        let y = (y - min) as i32;

        let y = y * graph_rect.size.height as i32 / (max - min) as i32;

        let x = x as i32 + graph_rect.top_left.x;
        let y = graph_rect.bottom_right().unwrap().y - y;
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

        let sample_index = i * chunk_size as u16;
        let is_integrated = sample_index
            > chart.len() as u16 - samples_since_start.unwrap_or(0) as u16
            && sample_index < chart.len() as u16 - samples_since_end.unwrap_or(0) as u16;

        let (x, y) = xy_to_coords(sample_index, avg);
        if is_integrated {
            display
                .fill_solid(
                    &Rectangle::with_corners(
                        Point::new(x, y),
                        Point::new(x, graph_rect.bottom_right().unwrap().y),
                    ),
                    Rgb565::CSS_DARK_SLATE_BLUE,
                )
                .unwrap();
        }
        display
            .fill_solid(
                &Rectangle::new(Point::new(x, y), Size::new(2, 2)),
                Rgb565::WHITE,
            )
            .unwrap();

        i += 1;
    }

    let graph_bottom = graph_rect.bottom_right().unwrap().y;
    if let Some(samples_since_start) = samples_since_start {
        let start_x = chart.len() - samples_since_start;
        if let Some(start_y) = chart.get(start_x) {
            let (x, y) = xy_to_coords(start_x as u16, *start_y);
            draw_cross(display.deref_mut(), Point::new(x, y), 2, Rgb565::GREEN);
            draw_triangle(
                display,
                Point::new(x, graph_bottom),
                10,
                true,
                Rgb565::GREEN,
            );
        }
    }

    if let Some(samples_since_end) = samples_since_end {
        let end_x = chart.len() - samples_since_end;
        if let Some(end_y) = chart.get(end_x) {
            let (x, y) = xy_to_coords(end_x as u16, *end_y);
            draw_cross(display.deref_mut(), Point::new(x, y), 2, Rgb565::RED);
            draw_triangle(display, Point::new(x, graph_bottom), 10, true, Rgb565::RED);
        }
    }
}

pub fn init_debug_ui(display: &mut Display<Spi<SPI1>>) {
    display.clear();

    SMALL_FONT
        .render(
            "Current value:",
            Point::new(10, 80),
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
    draw_chart(display, state.adc_history, 10, None, None, true);

    let mut s = String::<128>::default();
    s.clear();

    let variation = (state.max_adc_value - state.min_adc_value) / 2;

    uwrite!(s, "{} +-{}  ", state.adc_value, variation).unwrap();
    LARGE_DIGIT_FONT
        .render(
            &s[..],
            Point::new(10, 110),
            VerticalPosition::Top,
            FontColor::WithBackground {
                bg: Rgb565::WHITE,
                fg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();

    s.clear();
    let _ = uwrite!(s, "Samples: {}", state.sample_counter);
    TINY_FONT
        .render(
            &s[..],
            Point::new(50, 180),
            VerticalPosition::Top,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            &mut **display,
        )
        .unwrap();
}

pub async fn draw_boot_screen(display: &mut Display<Spi<SPI1>>) {
    let x = (display.width() / 2) as i32;
    let y = (display.height() / 2) as i32;

    draw_cross(display.deref_mut(), Point::new(x, y + 10), 20, Rgb565::RED);
    draw_badge(
        display,
        Point::new(x, y),
        " ",
        Rgb565::CSS_GRAY,
        Rgb565::BLACK,
    )
    .await;
    draw_badge(
        display,
        Point::new(x, y),
        " XXX ",
        Rgb565::WHITE,
        Rgb565::BLACK,
    )
    .await;
    draw_cross(
        display.deref_mut(),
        Point::new(x, y + 10),
        30,
        Rgb565::WHITE,
    );
    draw_badge(
        display,
        Point::new(x, y),
        env!("CARGO_PKG_VERSION"),
        Rgb565::BLACK,
        Rgb565::WHITE,
    )
    .await;
    Systick::delay(150.millis()).await;
}

const THICKENING_OFFSETS: [Point; 4] = [
    Point::new(0, 0),
    Point::new(1, 0),
    Point::new(0, 1),
    Point::new(1, 1),
];

pub fn draw_cross<D: AppDrawTarget>(display: &mut D, point: Point, size: u32, color: Rgb565) {
    for dir in [-1, 1] {
        for offset in THICKENING_OFFSETS {
            display
                .draw_iter(
                    (-(size as i32)..size as i32)
                        .map(|i| Pixel(offset + Point::new(point.x + i, point.y + i * dir), color)),
                )
                .unwrap();
        }
    }
}

pub fn draw_triangle(
    display: &mut Display<Spi<SPI1>>,
    point: Point,
    size: u32,
    upside_down: bool,
    color: Rgb565,
) {
    let sy = if upside_down { -1 } else { 1 };
    for offset in THICKENING_OFFSETS {
        for dir in [-1, 1] {
            display
                .draw_iter((0..size as i32).map(|i| {
                    Pixel(
                        offset + Point::new(point.x + dir * i, point.y - i * sy),
                        color,
                    )
                }))
                .unwrap();
        }
        display
            .draw_iter((-(size as i32)..size as i32).map(|i| {
                Pixel(
                    offset + Point::new(point.x + i, point.y - size as i32 * sy),
                    color,
                )
            }))
            .unwrap();
    }
}

pub async fn draw_badge(
    display: &mut Display<Spi<SPI1>>,
    point: Point,
    text: &str,
    fg: Rgb565,
    bg: Rgb565,
) {
    SMALL_FONT
        .render_aligned(
            text,
            point,
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground { fg: bg, bg: fg },
            &mut **display,
        )
        .unwrap();

    Systick::delay(100.millis()).await;

    SMALL_FONT
        .render_aligned(
            text,
            point,
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground { fg, bg },
            &mut **display,
        )
        .unwrap();
}

pub fn draw_panic_screen<D: AppDrawTarget>(display: &mut D, message: &str) {
    let width = display.bounding_box().size.width;
    let height = display.bounding_box().size.height;

    display
        .fill_solid(&display.bounding_box(), Rgb565::RED)
        .unwrap();

    for d in [-1, 0, 1] {
        draw_cross(
            display,
            Point::new(width as i32 / 2 + d * 40, 50),
            15,
            Rgb565::BLACK,
        );
    }

    TINY_FONT
        .render_aligned(
            env!("CARGO_PKG_VERSION"),
            Point::new(width as i32 / 2, 80),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground {
                fg: Rgb565::BLACK,
                bg: Rgb565::RED,
            },
            display,
        )
        .unwrap();

    SMALL_FONT
        .render_aligned(
            " FATAL ERROR ",
            Point::new(width as i32 / 2, 100),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground {
                fg: Rgb565::RED,
                bg: Rgb565::BLACK,
            },
            display,
        )
        .unwrap();

    let character_style = U8g2TextStyle::new(u8g2_font_profont17_mr, Rgb565::BLACK);

    let textbox_style = TextBoxStyleBuilder::new()
        .height_mode(HeightMode::FitToText)
        .alignment(embedded_text::alignment::HorizontalAlignment::Center)
        .build();

    let origin = Point::new(10, 150);
    let _ = TextBox::with_textbox_style(
        message,
        Rectangle::new(origin, Size::new(width - 20, height - origin.y as u32)),
        character_style,
        textbox_style,
    )
    .draw(display);
}
