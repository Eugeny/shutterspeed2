use core::fmt::{Debug, Write};

use app_measurements::TriggerThresholds;
use eg_seven_segment::SevenSegmentStyleBuilder;
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use embedded_graphics_framebuf::FrameBuf;
use heapless::{HistoryBuffer, String};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use ufmt::uwrite;

use super::{DrawFrameContext, Screen};
use crate::fonts::{SMALL_FONT, TINY_FONT};
use crate::primitives::Pointer;
use crate::{config as cfg, AppDrawTarget};

pub struct DebugScreen<DT, E> {
    adc_history: HistoryBuffer<u16, 1000>,
    is_triggered: bool,
    calibration: u16,
    threshold_low: u16,
    threshold_high: u16,
    max_value: u16,
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for DebugScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();
    }

    async fn draw_frame(&mut self, display: &mut DT, cx: DrawFrameContext) {
        let recent_samples = self.adc_history.len().min(10);
        let (avg_adc_value, min_adc_value, max_adc_value) = {
            let recent_iter = || {
                self.adc_history
                    .oldest_ordered()
                    .skip(self.adc_history.len() - recent_samples)
            };
            (
                (recent_iter().map(|x| *x as u32).sum::<u32>() / recent_samples as u32) as u16,
                *recent_iter().min().unwrap_or(&0),
                *recent_iter().max().unwrap_or(&0),
            )
        };

        let ll_origin = Point::new(display.bounding_box().size.width as i32 / 2, 60);
        self.draw_light_value(display, ll_origin, avg_adc_value);

        let bar_origin = Point::new(5, ll_origin.y);
        self.draw_bar(
            display,
            bar_origin,
            avg_adc_value,
            min_adc_value,
            max_adc_value,
        );

        let calibration_origin = bar_origin + Point::new(0, 40);
        self.draw_value(
            display,
            calibration_origin,
            " CALIBRATED TO ",
            self.calibration,
            cfg::COLOR_CALIBRATION,
        );

        let indicator_origin = calibration_origin + Point::new(100, 0);
        TINY_FONT
            .render_aligned(
                " OVER  ",
                indicator_origin,
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: cfg::COLOR_BACKGROUND,
                    bg: if self.is_triggered {
                        cfg::COLOR_RESULT_VALUE
                    } else {
                        cfg::COLOR_RESULT_VALUE_INACTIVE
                    },
                },
                display,
            )
            .unwrap();

        TINY_FONT
            .render_aligned(
                " UNDER ",
                indicator_origin + Point::new(0, 10),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: cfg::COLOR_BACKGROUND,
                    bg: if !self.is_triggered {
                        cfg::COLOR_RESULT_VALUE
                    } else {
                        cfg::COLOR_RESULT_VALUE_INACTIVE
                    },
                },
                display,
            )
            .unwrap();

        let noise_origin = calibration_origin + Point::new(0, 33);
        let noise = (max_adc_value - min_adc_value) / 2;
        self.draw_value(display, noise_origin, " NOISE ", noise, cfg::COLOR_NOISE);

        self.draw_value(
            display,
            noise_origin + Point::new(79, 0),
            " TRIG H ",
            self.threshold_high,
            cfg::COLOR_TRIGGER_HIGH,
        );

        self.draw_value(
            display,
            noise_origin + Point::new(37, 0),
            " TRIG L ",
            self.threshold_low,
            cfg::COLOR_TRIGGER_LOW,
        );
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> DebugScreen<DT, E> {
    pub fn new(calibration: u16, trigger_thresholds: TriggerThresholds, max_value: u16) -> Self {
        Self {
            adc_history: HistoryBuffer::new(),
            is_triggered: false,
            calibration,
            threshold_low: trigger_thresholds.trigger_low(calibration),
            threshold_high: trigger_thresholds.trigger_high(calibration),
            max_value,
            _phantom: core::marker::PhantomData,
        }
    }

    pub fn step(&mut self, adc_value: u16) {
        self.adc_history.write(adc_value);

        if !self.is_triggered && adc_value > self.threshold_high {
            self.is_triggered = true;
        }
        if self.is_triggered && adc_value < self.threshold_low {
            self.is_triggered = false;
        }
    }

    pub fn last_adc_value(&self) -> u16 {
        *self.adc_history.oldest_ordered().last().unwrap_or(&0)
    }

    fn draw_light_value(&mut self, display: &mut DT, origin: Point, avg_adc_values: u16) {
        let mut s = String::<128>::default();

        TINY_FONT
            .render_aligned(
                " LIGHT LEVEL ",
                origin + Point::new(0, -45),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    bg: cfg::COLOR_LEVEL,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        let large_style = SevenSegmentStyleBuilder::new()
            .digit_size(Size::new(16, 28)) // digits are 10x20 pixels
            .digit_spacing(2) // 5px spacing between digits
            .segment_width(4) // 5px wide segments
            .inactive_segment_color(cfg::COLOR_RESULT_VALUE_INACTIVE)
            .segment_color(cfg::COLOR_RESULT_VALUE) // active segments are green
            .build();

        s.clear();
        let rel_value = avg_adc_values as i32 - self.calibration as i32;
        if rel_value >= 0 {
            write!(s, " {:>4}", rel_value).unwrap();
        } else {
            write!(s, "-{:>4}", -rel_value).unwrap();
        }
        Text::with_alignment(
            &s[..],
            origin,
            large_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();
    }

    fn draw_bar(
        &mut self,
        display: &mut DT,
        origin: Point,
        avg_adc_value: u16,
        min_adc_value: u16,
        max_adc_value: u16,
    ) {
        const WIDTH: usize = 118;
        const HEIGHT: usize = 30;

        let mut buffer_data = [cfg::COLOR_BACKGROUND; WIDTH * HEIGHT];
        let mut buffer = FrameBuf::new(&mut buffer_data, WIDTH, HEIGHT);

        let scale = WIDTH as f32 / self.max_value as f32;
        let scale_value = |x: u16| (x as f32 * scale) as i32;

        let bar_y = 10;
        let bar_h = 10;

        let tick_w = 2;
        let tick_d = 3;

        // if max_adc_value.abs_diff(min_adc_value) > 2 {
        //     buffer
        //         .fill_contiguous(
        //             &Rectangle::new(
        //                 Point::new(scale_value(min_adc_value), 0),
        //                 Size::new(
        //                     scale_value(max_adc_value - min_adc_value) as u32 / 2 * 2 + 1,
        //                     5,
        //                 ),
        //             ),
        //             [cfg::COLOR_NOISE, Rgb565::CSS_DARK_ORANGE]
        //                 .iter()
        //                 .cycle()
        //                 .cloned(),
        //         )
        //         .unwrap();
        // }

        for i in 0..WIDTH as i32 / tick_d {
            let x = i * tick_d;
            let y = bar_y + if i % 2 == 0 { 0 } else { 1 };
            let value = (x as f32 / scale) as u16;
            // let value_next = ((x + tick_d) as f32 / scale) as u16;

            let color = if value < avg_adc_value {
                cfg::COLOR_RESULT_VALUE
            } else {
                cfg::COLOR_RESULT_VALUE_INACTIVE
            };

            // if value < self.calibration && self.calibration <= value_next {
            //     color = cfg::COLOR_CALIBRATION;
            // };

            // if value < self.threshold_high && self.threshold_high <= value_next {
            //     color = cfg::COLOR_TRIGGER_HIGH;
            // };

            // if value < self.threshold_low && self.threshold_low <= value_next {
            //     color = cfg::COLOR_TRIGGER_LOW;
            // };

            buffer
                .fill_solid(
                    &Rectangle::new(Point::new(x, y), Size::new(tick_w, bar_h)),
                    color,
                )
                .unwrap();

            if value > min_adc_value && value < max_adc_value {
                buffer
                    .fill_solid(
                        &Rectangle::new(
                            Point::new(x, y - 2 - tick_w as i32),
                            Size::new(tick_w, tick_w),
                        ),
                        cfg::COLOR_NOISE,
                    )
                    .unwrap();
            }
        }

        Pointer::new(
            Point::new(scale_value(self.calibration), bar_h as i32 + 13),
            5,
            true,
            cfg::COLOR_CALIBRATION,
        )
        .draw(&mut buffer)
        .unwrap();

        Pointer::new(
            Point::new(scale_value(self.threshold_low), bar_h as i32 + 13),
            5,
            true,
            cfg::COLOR_TRIGGER_LOW,
        )
        .draw(&mut buffer)
        .unwrap();

        Pointer::new(
            Point::new(scale_value(self.threshold_high), bar_h as i32 + 13),
            5,
            true,
            cfg::COLOR_TRIGGER_HIGH,
        )
        .draw(&mut buffer)
        .unwrap();

        display
            .fill_contiguous(
                &Rectangle::new(origin, Size::new(WIDTH as u32, HEIGHT as u32)),
                buffer_data,
            )
            .unwrap();
    }

    fn draw_value(
        &mut self,
        display: &mut DT,
        origin: Point,
        name: &str,
        value: u16,
        color: Rgb565,
    ) {
        let mut s = String::<128>::default();

        TINY_FONT
            .render(
                name,
                origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: color,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        s.clear();
        uwrite!(s, "{}", value).unwrap();
        SMALL_FONT
            .render(
                &s[..],
                origin + Point::new(1, 12),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: color,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();
    }
}
