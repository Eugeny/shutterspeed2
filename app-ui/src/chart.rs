use core::fmt::Debug;

use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use heapless::HistoryBuffer;
use micromath::F32Ext;

use crate::primitives::{Cross, Pointer};
use crate::{config as cfg, AppDrawTarget};

pub fn draw_chart<const LEN: usize, D: AppDrawTarget<E>, E: Debug>(
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
            Cross::new(Point::new(x, y), 5, cfg::COLOR_TRIGGER_HIGH)
                .draw(display)
                .unwrap();

            Pointer::new(
                Point::new(x, graph_bottom + 10),
                10,
                true,
                cfg::COLOR_TRIGGER_HIGH,
            )
            .draw(display)
            .unwrap();
        }
    }

    if let Some(samples_since_end) = samples_since_end {
        let end_x = chart.len() - samples_since_end;
        if let Some(end_y) = chart.get(end_x) {
            let (x, y) = xy_to_coords(end_x as u16, *end_y);
            Cross::new(Point::new(x, y), 5, cfg::COLOR_TRIGGER_LOW)
                .draw(display)
                .unwrap();
            Pointer::new(
                Point::new(x, graph_bottom + 10),
                10,
                true,
                cfg::COLOR_TRIGGER_LOW,
            )
            .draw(display)
            .unwrap();
        }
    }
}
