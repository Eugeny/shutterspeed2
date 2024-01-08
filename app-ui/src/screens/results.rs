use core::fmt::Debug;

use app_measurements::util::get_closest_shutter_speed;
use app_measurements::{CalibrationState, MeasurementResult};
use eg_seven_segment::SevenSegmentStyleBuilder;
use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::primitives::{Line, PrimitiveStyleBuilder, StyledDrawable};
use embedded_graphics::text::Text;
use embedded_graphics::Drawable;
use heapless::String;
use u8g2_fonts::types::{FontColor, VerticalPosition};
use ufmt::uwrite;

use super::Screen;
use crate::chart::draw_chart;
use crate::fonts::TINY_FONT;
use crate::format::write_fraction;
use crate::primitives::Pointer;
use crate::ruler::draw_speed_ruler;
use crate::{config as cfg, AppDrawTarget};

pub struct ResultsScreen<DT, E> {
    pub calibration: CalibrationState,
    pub result: MeasurementResult,
    _phantom: core::marker::PhantomData<(DT, E)>,
}

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for ResultsScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        display.clear(cfg::COLOR_BACKGROUND).unwrap();

        draw_chart(
            display,
            &self.result.sample_buffer,
            5,
            Some(self.result.samples_since_start),
            Some(self.result.samples_since_end),
            self.result.duration_micros,
            self.result.integrated_duration_micros,
            false,
        );
    }

    async fn draw_frame(&mut self, display: &mut DT) {
        let ss_origin = Point::new(display.bounding_box().center().x, 100);
        self.draw_shutter_speed(display, ss_origin);
        self.draw_deviation(display, ss_origin + Point::new(0, 130));

        draw_speed_ruler(
            display,
            Point::new(0, 290),
            self.result.integrated_duration_micros as f32 / 1_000_000.0,
        );
    }
}

fn micros_to_shutter_speed_str(micros: u64) -> String<128> {
    let mut s = String::<128>::default();
    if micros < 500_000 {
        write_fraction(&mut s, 1_000_000_f32 / micros as f32);
    } else {
        write_fraction(&mut s, micros as f32 / 1_000_000_f32);
    }
    s
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

        let is_inverse = duration_micros < 500_000;

        let small_style = SevenSegmentStyleBuilder::new()
            .digit_size(Size::new(12, 23)) // digits are 10x20 pixels
            .digit_spacing(5) // 5px spacing between digits
            .segment_width(3) // 5px wide segments
            .inactive_segment_color(cfg::COLOR_RESULT_VALUE_INACTIVE)
            .segment_color(cfg::COLOR_RESULT_VALUE) // active segments are green
            .build();
        let large_style = SevenSegmentStyleBuilder::new()
            .digit_size(Size::new(25, 45)) // digits are 10x20 pixels
            .digit_spacing(5) // 5px spacing between digits
            .segment_width(6) // 5px wide segments
            .inactive_segment_color(cfg::COLOR_RESULT_VALUE_INACTIVE)
            .segment_color(cfg::COLOR_RESULT_VALUE) // active segments are green
            .build();

        let number_origin = origin + Point::new(0, 60);
        let end_point = Text::with_alignment(
            &micros_to_shutter_speed_str(duration_micros)[..],
            number_origin,
            large_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        Text::new("5", end_point + Point::new(10, 0), small_style)
            .draw(display)
            .unwrap();

        if is_inverse {
            let one_ends = Text::new(
                "1",
                number_origin * 2 - end_point + Point::new(-25, -20),
                small_style,
            )
            .draw(display)
            .unwrap();

            Line::new(one_ends, one_ends + Point::new(7, -20))
                .draw_styled(
                    &PrimitiveStyleBuilder::new()
                        .stroke_width(2)
                        .stroke_color(cfg::COLOR_RESULT_VALUE)
                        .build(),
                    display,
                )
                .unwrap();
        }

        TINY_FONT
            .render_aligned(
                " Shutter speed ",
                origin + Point::new(0, -10),
                VerticalPosition::Top,
                u8g2_fonts::types::HorizontalAlignment::Center,
                FontColor::WithBackground {
                    bg: cfg::COLOR_RESULT_VALUE,
                    fg: cfg::COLOR_BACKGROUND,
                },
                display,
            )
            .unwrap();
    }

    fn draw_deviation(&mut self, display: &mut DT, origin: Point) {
        let best_match_duration =
            get_closest_shutter_speed(self.result.integrated_duration_micros as f32 / 1_000_000.0);

        let percent_offset = ((self.result.integrated_duration_micros as f32 / 1_000_000.0
            - best_match_duration)
            / best_match_duration
            * 100.0) as i16;

        let color = if percent_offset.abs() < 15 {
            cfg::COLOR_RESULT_GOOD
        } else if percent_offset.abs() < 30 {
            cfg::COLOR_RESULT_FAIR
        } else {
            cfg::COLOR_RESULT_BAD
        };

        TINY_FONT
            .render_aligned(
                " Lag ",
                origin + Point::new(0, -45),
                VerticalPosition::Top,
                u8g2_fonts::types::HorizontalAlignment::Center,
                FontColor::WithBackground {
                    bg: color,
                    fg: cfg::COLOR_BACKGROUND,
                },
                display,
            )
            .unwrap();

        let small_style = SevenSegmentStyleBuilder::new()
            .digit_size(Size::new(12, 23)) // digits are 10x20 pixels
            .digit_spacing(5) // 5px spacing between digits
            .segment_width(3) // 5px wide segments
            .inactive_segment_color(cfg::COLOR_RESULT_VALUE_INACTIVE)
            .segment_color(color) // active segments are green
            .build();

        let mut s = String::<128>::default();
        uwrite!(s, "{}", percent_offset.abs()).unwrap();

        let end_point = Text::with_alignment(
            &s[..],
            origin,
            small_style,
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        let percent_end_point = Text::with_alignment(
            "°o",
            end_point + Point::new(15, 0),
            SevenSegmentStyleBuilder::from(&small_style)
                .digit_spacing(3)
                .build(),
            embedded_graphics::text::Alignment::Center,
        )
        .draw(display)
        .unwrap();

        Line::new(
            percent_end_point + Point::new(-27, 0),
            percent_end_point + Point::new(-7, -20),
        )
        .draw_styled(
            &PrimitiveStyleBuilder::new()
                .stroke_width(3)
                .stroke_color(color)
                .build(),
            display,
        )
        .unwrap();

        Pointer::new(
            origin * 2 - end_point + Point::new(-10, if percent_offset > 0 { -15 } else { -5 }),
            5,
            percent_offset > 0,
            cfg::COLOR_RESULT_VALUE,
        )
        .draw(display)
        .unwrap();
    }
}
