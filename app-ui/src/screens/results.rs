use core::fmt::Debug;

use app_measurements::util::get_closest_shutter_speed;
use app_measurements::{CalibrationState, MeasurementResult};
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::Drawable;
use heapless::String;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use ufmt::uwrite;

use super::Screen;
use crate::chart::draw_chart;
use crate::fonts::{LARGE_DIGIT_FONT, SMALLER_FONT, SMALL_FONT, TINY_FONT};
use crate::format::write_fraction;
use crate::primitives::Pointer;
use crate::ruler::draw_speed_ruler;
use crate::AppDrawTarget;

pub struct ResultsScreen<DT, E> {
    pub calibration: CalibrationState,
    pub result: MeasurementResult,
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for ResultsScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

        draw_speed_ruler(
            display,
            Point::new(0, 290),
            self.result.integrated_duration_micros as f32 / 1_000_000.0,
        );

        draw_chart(
            display,
            &self.result.sample_buffer,
            5,
            Some(self.result.samples_since_start),
            Some(self.result.samples_since_end),
            false,
        );
    }

    async fn draw_frame(&mut self, display: &mut DT) {
        let ss_origin = Point::new(10, 100);
        self.draw_shutter_speed(display, ss_origin);

        let deviation_origin = ss_origin + Point::new(0, 90);
        self.draw_deviation(display, deviation_origin);

        let exposure_time_origin = deviation_origin + Point::new(80, 0);
        self.draw_exposure_time(display, exposure_time_origin);
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> ResultsScreen<DT, E> {
    pub fn new(calibration: CalibrationState, result: MeasurementResult) -> Self {
        Self {
            calibration,
            result,
            _phantom: core::marker::PhantomData,
        }
    }

    fn draw_shutter_speed(&mut self, display: &mut DT, origin: Point) {
        let duration_micros = self.result.integrated_duration_micros.max(1);
        let mut s = String::<128>::default();

        let dim = if duration_micros < 500_000 {
            write_fraction(&mut s, 1_000_000_f32 / duration_micros as f32);
            SMALL_FONT
                .render(
                    "1/",
                    origin + Point::new(5, 30),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::CSS_LIGHT_GRAY,
                        bg: Rgb565::BLACK,
                    },
                    display,
                )
                .unwrap();
            LARGE_DIGIT_FONT
                .render(
                    &s[..],
                    origin + Point::new(35, 15),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::WHITE,
                        bg: Rgb565::BLACK,
                    },
                    display,
                )
                .unwrap()
        } else {
            write_fraction(&mut s, duration_micros as f32 / 1_000_000_f32);
            LARGE_DIGIT_FONT
                .render(
                    &s[..],
                    origin + Point::new(5, 15),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::WHITE,
                        bg: Rgb565::BLACK,
                    },
                    display,
                )
                .unwrap()
        };
        SMALL_FONT
            .render(
                "s",
                dim.bounding_box.unwrap().bottom_right().unwrap() + Point::new(5, -25),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: Rgb565::CSS_LIGHT_GRAY,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        TINY_FONT
            .render(
                " Shutter speed ",
                origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: Rgb565::WHITE,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();
    }

    fn draw_exposure_time(&mut self, display: &mut DT, origin: Point) {
        TINY_FONT
            .render(
                " Exposure time ",
                origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: Rgb565::CSS_ORANGE_RED,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        {
            let mut s = String::<128>::default();

            let micros = self.result.integrated_duration_micros;
            let label = if micros > 10000 {
                let millis = micros / 1000;
                uwrite!(s, "{}", millis).unwrap();
                "ms"
            } else {
                uwrite!(s, "{}", micros).unwrap();
                "us"
            };

            let dim = SMALL_FONT
                .render(
                    &s[..],
                    origin + Point::new(5, 25),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::WHITE,
                        bg: Rgb565::BLACK,
                    },
                    display,
                )
                .unwrap();

            TINY_FONT
                .render(
                    label,
                    dim.bounding_box.unwrap().bottom_right().unwrap() + Point::new(5, -16),
                    VerticalPosition::Top,
                    FontColor::WithBackground {
                        fg: Rgb565::CSS_LIGHT_GRAY,
                        bg: Rgb565::BLACK,
                    },
                    display,
                )
                .unwrap();
        }
    }

    fn draw_deviation(&mut self, display: &mut DT, origin: Point) {
        let best_match_duration =
            get_closest_shutter_speed(self.result.integrated_duration_micros as f32 / 1_000_000.0);

        let percent_offset = ((self.result.integrated_duration_micros as f32 / 1_000_000.0
            - best_match_duration)
            / best_match_duration
            * 100.0) as i16;

        let color = if percent_offset.abs() < 15 {
            Rgb565::CSS_PALE_GREEN
        } else if percent_offset.abs() < 30 {
            Rgb565::CSS_ORANGE_RED
        } else {
            Rgb565::CSS_RED
        };

        TINY_FONT
            .render(
                " Lag ",
                origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: color,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        let mut s = String::<128>::default();
        uwrite!(s, "{}", percent_offset.abs()).unwrap();

        let dim = SMALL_FONT
            .render(
                &s[..],
                origin + Point::new(20, 25),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: color,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();
        SMALLER_FONT
            .render(
                "%",
                dim.bounding_box.unwrap().bottom_right().unwrap() + Point::new(3, -18),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: color,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        Pointer::new(
            origin + Point::new(6, if percent_offset > 0 { 30 } else { 40 }),
            5,
            percent_offset > 0,
            Rgb565::WHITE,
        )
        .draw(display)
        .unwrap();
    }
}
