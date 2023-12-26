use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use heapless::String;
use u8g2_fonts::types::{FontColor, VerticalPosition};

use super::Screen;
use crate::display::AppDrawTarget;
use crate::format::{write_fraction, write_micros};
use crate::measurement::{CalibrationState, MeasurementResult};
use crate::ui::fonts::{LARGE_DIGIT_FONT, SMALL_FONT, TINY_FONT};
use crate::ui::{draw_chart, draw_speed_ruler};

pub struct ResultsScreen {
    pub calibration: CalibrationState,
    pub result: MeasurementResult,
}

impl Screen for ResultsScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        let exposure_time_origin = Point::new(20, 100);
        {
            let duration_micros = self.result.integrated_duration_micros.max(1);
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
                        display,
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
                        display,
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
                        display,
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
                    display,
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
                    display,
                )
                .unwrap();
        }

        {
            let mut s = String::<128>::default();
            s.push(' ').unwrap();
            write_micros(&mut s, self.result.integrated_duration_micros);
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
                    display,
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
        //         display,
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
        //     display,
        // ).unwrap();
        // }

        draw_speed_ruler(
            display,
            Point::new(0, 280),
            self.result.integrated_duration_micros as f32 / 1_000_000.0,
        );

        draw_chart(
            display,
            &self.result.sample_buffer,
            25,
            Some(self.result.samples_since_start),
            Some(self.result.samples_since_end),
            false,
        );
    }
}
