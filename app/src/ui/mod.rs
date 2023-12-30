pub mod fonts;
pub mod primitives;
pub mod screens;

use core::ops::DerefMut;

use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use embedded_text::style::{HeightMode, TextBoxStyleBuilder};
use embedded_text::TextBox;
use hal::prelude::*;
use heapless::{HistoryBuffer, String};
use micromath::F32Ext;
use rtic_monotonics::systick::Systick;
use stm32f4xx_hal as hal;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::U8g2TextStyle;
use ufmt::uwrite;

use self::fonts::{TinyFont, SMALL_FONT, TINY_FONT};
use self::primitives::{Cross, Pointer};
use crate::display::AppDrawTarget;
use crate::hardware_config as hw;
use crate::hardware_config::DisplayType;

const KNOWN_SHUTTER_DURATIONS: [f32; 18] = [
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

pub fn get_closest_shutter_speed(duration: f32) -> f32 {
    let mut best_match = 1.0;
    for d in KNOWN_SHUTTER_DURATIONS.iter() {
        if (d - duration).abs() < (best_match - duration).abs() {
            best_match = *d;
        }
    }
    best_match
}

fn draw_speed_ruler<D: AppDrawTarget>(display: &mut D, origin: Point, actual_duration_secs: f32) {
    let width = display.bounding_box().size.width;
    let ruler_height = 10;

    let duration_to_x_offset = |d: f32| ((1.0 / d).log2() * 60.0) as i32;

    let actual_x = origin.x + duration_to_x_offset(actual_duration_secs);

    let overall_x_offset = width as i32 / 2 - actual_x;

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

    let best_match = get_closest_shutter_speed(actual_duration_secs);

    for duration in KNOWN_SHUTTER_DURATIONS.iter() {
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
            Pointer::new(Point::new(x, y - ruler_height - 1), 10, false, color)
                .draw(display)
                .unwrap();
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
                display,
            )
            .unwrap();
    }

    Pointer::new(
        Point::new(overall_x_offset + actual_x - 2, origin.y - ruler_height - 1),
        12,
        false,
        Rgb565::WHITE,
    )
    .draw(display)
    .unwrap();
}

fn draw_chart<const LEN: usize, D: AppDrawTarget>(
    display: &mut D,
    chart: &HistoryBuffer<u16, LEN>,
    graph_y: i32,
    samples_since_start: Option<usize>,
    samples_since_end: Option<usize>,
    clear: bool,
) {
    let padding = 10;

    let len = chart.len();
    let width = display.bounding_box().size.width - padding * 2;
    let graph_rect = Rectangle::new(Point::new(padding as i32, graph_y), Size::new(width, 40));

    if clear {
        display.fill_solid(&graph_rect, Rgb565::BLACK).unwrap();
    }

    let mut y_min = chart.iter().min().cloned().unwrap_or(0);
    let mut y_max = chart.iter().max().cloned().unwrap_or(0).max(y_min + 1);

    if (y_max - y_min) < 10 {
        y_max = y_max.saturating_add(50);
        y_min = y_min.saturating_sub(50)
    }

    let chunk_size = ((len as f32 / width as f32).ceil() as u32).max(1);
    let mut i = 0;
    let mut done = false;
    let mut iter = chart.oldest_ordered();

    let xy_to_coords = |x: u16, y: u16| {
        let x = x / chunk_size as u16;
        let y = (y - y_min) as i32;

        let y = y * graph_rect.size.height as i32 / (y_max - y_min) as i32;

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
                    Rgb565::CSS_DARK_RED,
                )
                .unwrap();
        }
        display
            .fill_solid(
                &Rectangle::new(Point::new(x, y), Size::new(2, 2)),
                Rgb565::RED,
            )
            .unwrap();

        i += 1;
    }

    let graph_bottom = graph_rect.bottom_right().unwrap().y;
    if let Some(samples_since_start) = samples_since_start {
        let start_x = chart.len() - samples_since_start;
        if let Some(start_y) = chart.get(start_x) {
            let (x, y) = xy_to_coords(start_x as u16, *start_y);
            Cross::new(Point::new(x, y), 5, hw::COLOR_TRIGGER_HIGH)
                .draw(display)
                .unwrap();

            Pointer::new(
                Point::new(x, graph_bottom + 10),
                10,
                true,
                hw::COLOR_TRIGGER_HIGH,
            )
            .draw(display)
            .unwrap();
        }
    }

    if let Some(samples_since_end) = samples_since_end {
        let end_x = chart.len() - samples_since_end;
        if let Some(end_y) = chart.get(end_x) {
            let (x, y) = xy_to_coords(end_x as u16, *end_y);
            Cross::new(Point::new(x, y), 5, hw::COLOR_TRIGGER_LOW)
                .draw(display)
                .unwrap();
            Pointer::new(
                Point::new(x, graph_bottom + 10),
                10,
                true,
                hw::COLOR_TRIGGER_LOW,
            )
            .draw(display)
            .unwrap();
        }
    }
}

pub async fn draw_boot_screen(display: &mut DisplayType) {
    let x = (display.width() / 2) as i32;
    let y = (display.height() / 2) as i32;

    Cross::new(Point::new(x, y + 10), 20, Rgb565::RED)
        .draw(display.deref_mut())
        .unwrap();
    draw_badge(
        &mut **display,
        Point::new(x, y),
        " ",
        Rgb565::CSS_GRAY,
        Rgb565::BLACK,
    )
    .await;
    draw_badge(
        &mut **display,
        Point::new(x, y),
        " XXX ",
        Rgb565::WHITE,
        Rgb565::BLACK,
    )
    .await;
    Cross::new(Point::new(x, y + 10), 30, Rgb565::WHITE)
        .draw(display.deref_mut())
        .unwrap();
    draw_badge(
        &mut **display,
        Point::new(x, y),
        env!("CARGO_PKG_VERSION"),
        Rgb565::BLACK,
        Rgb565::WHITE,
    )
    .await;
    Systick::delay(150.millis()).await;
}

pub async fn draw_badge<D: AppDrawTarget>(
    display: &mut D,
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
            display,
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
            display,
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
        let _ =
            Cross::new(Point::new(width as i32 / 2 + d * 40, 50), 15, Rgb565::BLACK).draw(display);
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

    let character_style = U8g2TextStyle::new(TinyFont {}, Rgb565::BLACK);

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
