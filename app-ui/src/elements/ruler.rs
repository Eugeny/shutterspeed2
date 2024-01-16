use core::fmt::Debug;

use app_measurements::util::{get_closest_shutter_speed, KNOWN_SHUTTER_DURATIONS};
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use heapless::String;
#[cfg(feature = "cortex-m")]
use micromath::F32Ext;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use ufmt::uwrite;

use crate::fonts::TINY_FONT;
use crate::{config as cfg, AppDrawTarget};

pub fn draw_speed_ruler<D: AppDrawTarget<E>, E: Debug>(
    display: &mut D,
    origin: Point,
    actual_duration_secs: f32,
) {
    let width = display.bounding_box().size.width;
    let ruler_height = 5;

    let duration_to_x_offset = |d: f32| ((1.0 / d).log2() * 30.0) as i32;

    let actual_x = origin.x + duration_to_x_offset(actual_duration_secs);

    let overall_x_offset = width as i32 / 2 - actual_x;

    display
        .fill_contiguous(
            &Rectangle::new(
                origin - Point::new(0, ruler_height),
                Size::new(width - 1, ruler_height as u32),
            ),
            [
                cfg::COLOR_RULER,
                cfg::COLOR_BACKGROUND,
                cfg::COLOR_BACKGROUND,
                cfg::COLOR_BACKGROUND,
            ]
            .iter()
            .cycle()
            .cloned(),
        )
        .unwrap();

    display
        .fill_solid(
            &Rectangle::new(origin, Size::new(width, 1)),
            cfg::COLOR_RULER,
        )
        .unwrap();
    display
        .fill_solid(
            &Rectangle::new(origin + Point::new(0, -ruler_height), Size::new(width, 1)),
            cfg::COLOR_RULER,
        )
        .unwrap();

    let best_match = get_closest_shutter_speed(actual_duration_secs);

    for (duration, bottom) in KNOWN_SHUTTER_DURATIONS
        .iter()
        .map(|x| (x, true))
        .chain([(&actual_duration_secs, false)].iter().copied())
    {
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

        if actual_duration_secs == *duration {
            color = cfg::COLOR_RESULT_VALUE;
        }
        if best_match == *duration {
            color = cfg::COLOR_NEAREST_SPEED;
        }

        let label_size = TINY_FONT
            .get_rendered_dimensions(&s[..], Point::zero(), VerticalPosition::Top)
            .unwrap();
        let label_origin = Point::new(
            x - label_size.bounding_box.unwrap().size.width as i32 / 2,
            if bottom { y + 3 } else { y - ruler_height - 11 },
        );

        let label_off_screen = label_origin.x + label_size.bounding_box.unwrap().size.width as i32
            > width as i32
            || label_origin.x < 0;

        if x > 1 && x < width as i32 - 2 {
            display
                .fill_solid(
                    &Rectangle::new(
                        Point::new(x - 1, y - ruler_height)
                            + if bottom {
                                Point::zero()
                            } else {
                                Point::new(0, -1)
                            },
                        Size::new(
                            2,
                            ruler_height as u32 + if label_off_screen { 0 } else { 2 },
                        ),
                    ),
                    color,
                )
                .unwrap();
        }

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

    // Pointer::new(
    //     Point::new(overall_x_offset + actual_x - 2, origin.y - ruler_height - 1),
    //     12,
    //     false,
    //     Rgb565::WHITE,
    // )
    // .draw(display)
    // .unwrap();
}
