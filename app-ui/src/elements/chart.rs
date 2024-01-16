use core::fmt::Debug;

use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::primitives::{Line, Primitive, PrimitiveStyleBuilder, Rectangle};
use embedded_graphics::Drawable;
use heapless::{HistoryBuffer, String};
#[cfg(feature = "cortex-m")]
use micromath::F32Ext;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use ufmt::uwrite;

use crate::config::COLOR_BACKGROUND;
use crate::fonts::TINY_FONT;
use crate::{config as cfg, AppDrawTarget};

#[allow(clippy::too_many_arguments)]
pub fn draw_chart<const LEN: usize, D: AppDrawTarget<E>, E: Debug>(
    display: &mut D,
    chart: &HistoryBuffer<u16, LEN>,
    graph_y: i32,
    samples_since_start: Option<usize>,
    samples_since_end: Option<usize>,
    raw_micros: u64,
    integrated_micros: u64,
    clear: bool,
) {
    let padding = 10;

    let len = chart.len();
    let max_width = display.bounding_box().size.width - padding * 2;

    let mut y_min = *chart.iter().min().unwrap_or(&0);
    let mut y_max = *chart.iter().max().unwrap_or(&0).max(&(y_min + 1));

    // Scale y down if the chart is super flat
    if (y_max - y_min) < 10 {
        y_max = y_max.saturating_add(50);
        y_min = y_min.saturating_sub(50)
    }

    // Leave some space below the baseline
    y_min = y_min.saturating_sub((y_max - y_min) / 5);

    let chunk_size = ((len as f32 / max_width as f32).ceil() as u32).max(1);
    let mut i = 0;
    let mut done = false;
    let mut iter = chart.oldest_ordered();

    // Center the chart
    let width = chart.len() as u32 / chunk_size;

    let graph_rect = Rectangle::new(
        Point::new(
            (display.bounding_box().size.width / 2 - width / 2) as i32,
            graph_y,
        ),
        Size::new(width, 20),
    );

    if clear {
        display
            .fill_solid(&graph_rect, cfg::COLOR_BACKGROUND)
            .unwrap();
    }

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

        display
            .fill_solid(
                &Rectangle::with_corners(
                    Point::new(x, y),
                    Point::new(x, graph_rect.bottom_right().unwrap().y),
                ),
                if is_integrated {
                    cfg::COLOR_CHART_2
                } else {
                    cfg::COLOR_CHART_1
                },
            )
            .unwrap();
        display
            .fill_solid(
                &Rectangle::new(Point::new(x, y), Size::new(2, 2)),
                if is_integrated {
                    cfg::COLOR_CHART_3
                } else {
                    cfg::COLOR_CHART_2
                },
            )
            .unwrap();

        i += 1;
    }

    let mut start_x = None;
    let mut end_x = None;

    let graph_bottom = graph_rect.bottom_right().unwrap().y;
    if let Some(samples_since_start) = samples_since_start {
        let start_idx = chart.len() - samples_since_start;
        if let Some(start_y) = chart.get(start_idx) {
            start_x = Some(xy_to_coords(start_idx as u16, *start_y).0);
        }
    }

    if let Some(samples_since_end) = samples_since_end {
        let end_idx = chart.len() - samples_since_end;
        if let Some(end_y) = chart.get(end_idx) {
            end_x = Some(xy_to_coords(end_idx as u16, *end_y).0);
        }
    }

    if let (Some(start_x), Some(end_x)) = (start_x, end_x) {
        let start_x = start_x.min(end_x);
        let end_x = start_x.max(end_x);

        let line_y = graph_bottom + 7;

        let line_style = PrimitiveStyleBuilder::new()
            .stroke_color(cfg::COLOR_CHART_2)
            .stroke_width(1)
            .build();

        Line::new(Point::new(start_x, line_y), Point::new(end_x, line_y))
            .into_styled(line_style)
            .draw(display)
            .unwrap();

        for x in [start_x, end_x] {
            Line::new(Point::new(x, line_y - 3), Point::new(x, line_y + 3))
                .into_styled(line_style)
                .draw(display)
                .unwrap();
        }

        // display
        //     .fill_solid(
        //         &Rectangle::with_corners(Point::new(start_x, start_y), Point::new(end_x, end_y)),
        //         cfg::COLOR_CHART_3,
        //     )
        //     .unwrap();

        TINY_FONT
            .with_line_height(20)
            .render_aligned(
                &micros_to_string(raw_micros)[..],
                Point::new(
                    (start_x + end_x) / 2,
                    line_y + TINY_FONT.get_ascent() as i32 / 2,
                ),
                VerticalPosition::Baseline,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: cfg::COLOR_CHART_3,
                    bg: cfg::COLOR_BACKGROUND,
                },
                display,
            )
            .unwrap();

        TINY_FONT
            .with_line_height(20)
            .render_aligned(
                &micros_to_string(integrated_micros)[..],
                Point::new(graph_rect.center().x, graph_rect.bottom_right().unwrap().y),
                VerticalPosition::Bottom,
                HorizontalAlignment::Center,
                FontColor::Transparent(COLOR_BACKGROUND),
                display,
            )
            .unwrap();
    }
}

fn micros_to_string(micros: u64) -> String<128> {
    let mut s = String::<128>::default();

    if micros > 10000 {
        let millis = micros / 1000;
        uwrite!(s, " {} ms ", millis).unwrap();
    } else {
        uwrite!(s, " {} us ", micros).unwrap();
    };

    s
}
