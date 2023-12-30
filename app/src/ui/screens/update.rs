use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use embedded_graphics::Drawable;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use super::Screen;
use crate::display::AppDrawTarget;
use crate::ui::fonts::{SMALL_FONT, TINY_FONT};
use crate::ui::primitives::Cross;

pub struct UpdateScreen {}
const COLOR: Rgb565 = Rgb565::BLUE;

impl Screen for UpdateScreen {
    async fn draw_init<DT: AppDrawTarget>(&mut self, display: &mut DT) {
        let width = display.bounding_box().size.width;
        let height = display.bounding_box().size.height;

        display.fill_solid(&display.bounding_box(), COLOR).unwrap();

        for d in [-1, 0, 1] {
            let _ = Cross::new(Point::new(width as i32 / 2 + d * 40, 50), 15, Rgb565::BLACK)
                .draw(display);
        }

        TINY_FONT
            .render_aligned(
                env!("CARGO_PKG_VERSION"),
                Point::new(width as i32 / 2, 80),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: Rgb565::BLACK,
                    bg: COLOR,
                },
                display,
            )
            .unwrap();

        SMALL_FONT
            .render_aligned(
                " UPDATE MODE ",
                Point::new(width as i32 / 2, 100),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: COLOR,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        TINY_FONT
            .render_aligned(
                " USB DFU MODE ACTIVE ",
                Point::new(width as i32 / 2, height as i32 - 50),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: Rgb565::CYAN,
                    bg: Rgb565::BLACK,
                },
                display,
            )
            .unwrap();

        bootloader_api::reboot_into_bootloader()
    }

    async fn draw_frame<DT: AppDrawTarget>(&mut self, _display: &mut DT) {}
}
