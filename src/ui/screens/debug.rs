use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use heapless::{HistoryBuffer, String};
use u8g2_fonts::types::{FontColor, VerticalPosition};
use ufmt::uwrite;

use super::Screen;
use crate::display::AppDrawTarget;
use crate::ui::draw_chart;
use crate::ui::fonts::{LARGE_DIGIT_FONT, SMALL_FONT, TINY_FONT};

pub struct DebugUiState {
    pub adc_value: u16,
    pub min_adc_value: u16,
    pub max_adc_value: u16,
    pub adc_history: HistoryBuffer<u16, 100>,
    pub sample_counter: u32,
}

pub struct DebugScreen {
    pub state: DebugUiState,
}

impl Screen for DebugScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        display.clear(Rgb565::BLACK).unwrap();

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
                display,
            )
            .unwrap();
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        draw_chart(display, &self.state.adc_history, 10, None, None, true);

        let mut s = String::<128>::default();
        s.clear();

        let variation = (self.state.max_adc_value - self.state.min_adc_value) / 2;

        uwrite!(s, "{} +-{}  ", self.state.adc_value, variation).unwrap();
        LARGE_DIGIT_FONT
            .render(
                &s[..],
                Point::new(10, 110),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    bg: Rgb565::WHITE,
                    fg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        s.clear();
        let _ = uwrite!(s, "Samples: {}", self.state.sample_counter);
        TINY_FONT
            .render(
                &s[..],
                Point::new(50, 180),
                VerticalPosition::Top,
                FontColor::WithBackground {
                    fg: Rgb565::RED,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();
    }
}
