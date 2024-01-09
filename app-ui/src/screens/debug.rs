use core::fmt::Debug;

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use embedded_graphics_framebuf::FrameBuf;
use heapless::{HistoryBuffer, String};
use u8g2_fonts::types::{FontColor, VerticalPosition};
use ufmt::uwrite;

use super::Screen;
// use crate::hardware_config as cfg;
use crate::fonts::{LARGE_DIGIT_FONT, SMALL_FONT, TINY_FONT};
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

    async fn draw_frame(&mut self, display: &mut DT) {
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

        // draw_chart(display, &self.state.adc_history, 10, None, None, true);

        let ll_origin = Point::new(15, 20);
        self.draw_light_value(display, ll_origin, avg_adc_value);

        let bar_origin = ll_origin + Point::new(0, 90);
        self.draw_bar(
            display,
            bar_origin,
            avg_adc_value,
            min_adc_value,
            max_adc_value,
        );

        let calibration_origin = bar_origin + Point::new(0, 60);
        self.draw_value(
            display,
            calibration_origin,
            " CALIBRATED TO ",
            self.calibration,
            cfg::COLOR_CALIBRATION,
        );

        Pointer::new(
            calibration_origin + Point::new(180, 5),
            20,
            true,
            if self.is_triggered {
                cfg::COLOR_TRIGGER_HIGH
            } else {
                cfg::COLOR_TRIGGER_LOW
            },
        )
        .draw(display)
        .unwrap();

        let noise_origin = calibration_origin + Point::new(0, 65);
        let noise = (max_adc_value - min_adc_value) / 2;
        self.draw_value(display, noise_origin, " NOISE ", noise, cfg::COLOR_NOISE);

        self.draw_value(
            display,
            noise_origin + Point::new(150, 0),
            " TRIG H ",
            self.threshold_high,
            cfg::COLOR_TRIGGER_HIGH,
        );

        self.draw_value(
            display,
            noise_origin + Point::new(70, 0),
            " TRIG L ",
            self.threshold_low,
            cfg::COLOR_TRIGGER_LOW,
        );
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> DebugScreen<DT, E> {
    pub fn new(
        calibration: u16,
        trigger_threshold_low: f32,
        trigger_threshold_high: f32,
        max_value: u16,
    ) -> Self {
        Self {
            adc_history: HistoryBuffer::new(),
            is_triggered: false,
            calibration,
            threshold_low: (calibration as f32 * trigger_threshold_low) as u16,
            threshold_high: (calibration as f32 * trigger_threshold_high) as u16,
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

    fn draw_light_value(&mut self, display: &mut DT, origin: Point, avg_adc_values: u16) {
        let mut s = String::<128>::default();

        TINY_FONT
            .render(
                " LIGHT LEVEL ",
                origin,
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: cfg::COLOR_LEVEL,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        s.clear();
        let rel_value = avg_adc_values as i32 - self.calibration as i32;
        if rel_value > 0 {
            uwrite!(s, "+{}  ", rel_value).unwrap();
        } else {
            uwrite!(s, "-{}  ", -rel_value).unwrap();
        }
        LARGE_DIGIT_FONT
            .render(
                &s[..],
                origin + Point::new(1, 15),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: Rgb565::WHITE,
                    bg: Rgb565::BLACK,
                },
                display,
            )
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
        const WIDTH: usize = 200;
        const HEIGHT: usize = 40;

        let mut buffer_data = [Rgb565::BLACK; WIDTH * HEIGHT];
        let mut buffer = FrameBuf::new(&mut buffer_data, WIDTH, HEIGHT);

        let scale = WIDTH as f32 / self.max_value as f32;
        let scale_value = |x: u16| (x as f32 * scale) as i32;

        let bar_y = 10;
        let bar_h = 5;
        buffer
            .fill_contiguous(
                &Rectangle::new(Point::new(0, bar_y), Size::new(WIDTH as u32 - 1, bar_h)),
                [Rgb565::CSS_DARK_GREEN, Rgb565::BLACK]
                    .iter()
                    .cycle()
                    .cloned(),
            )
            .unwrap();

        buffer
            .fill_contiguous(
                &Rectangle::new(
                    Point::new(scale_value(min_adc_value), bar_y),
                    Size::new(
                        scale_value(max_adc_value - min_adc_value).max(1) as u32 / 2 * 2 + 1,
                        bar_h,
                    ),
                ),
                [cfg::COLOR_NOISE, Rgb565::CSS_DARK_ORANGE]
                    .iter()
                    .cycle()
                    .cloned(),
            )
            .unwrap();

        Pointer::new(
            Point::new(scale_value(avg_adc_value), 10),
            10,
            false,
            cfg::COLOR_LEVEL,
        )
        .draw(&mut buffer)
        .unwrap();

        Pointer::new(
            Point::new(scale_value(self.calibration), 15),
            10,
            true,
            cfg::COLOR_CALIBRATION,
        )
        .draw(&mut buffer)
        .unwrap();

        Pointer::new(
            Point::new(scale_value(self.threshold_low), 15),
            10,
            true,
            cfg::COLOR_TRIGGER_LOW,
        )
        .draw(&mut buffer)
        .unwrap();

        Pointer::new(
            Point::new(scale_value(self.threshold_high), 15),
            10,
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
                origin + Point::new(1, 25),
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
