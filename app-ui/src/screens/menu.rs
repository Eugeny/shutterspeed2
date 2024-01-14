use core::fmt::Debug;

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use super::Screen;
use crate::fonts::{SMALL_FONT, TINY_FONT};
use crate::{config, AppDrawTarget};

pub struct MenuScreen<DT, E> {
    pub position: usize,
    pub sensitivity: u8,
    last_position: usize,
    _phantom: core::marker::PhantomData<(DT, E)>,
}

const LABELS: [&str; 4] = [" MEASURE ", " DEBUG ", " SENSITIVITY ", " UPDATE "];
const SENSITIVITY_LABELS: [&str; 3] = [" LO ", " MED ", " HI "];

impl<DT: AppDrawTarget<E>, E: Debug> Screen<DT, E> for MenuScreen<DT, E> {
    async fn draw_init(&mut self, display: &mut DT) {
        let width = display.bounding_box().size.width;
        let height = display.bounding_box().size.height;

        display
            .fill_solid(&display.bounding_box(), config::COLOR_BACKGROUND)
            .unwrap();

        TINY_FONT
            .render_aligned(
                env!("CARGO_PKG_VERSION"),
                Point::new(width as i32 / 2, height as i32 - 30),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
                FontColor::WithBackground {
                    fg: Rgb565::BLACK,
                    bg: Rgb565::CSS_GRAY,
                },
                display,
            )
            .unwrap();
    }

    async fn draw_frame(&mut self, display: &mut DT) {
        let bg = config::COLOR_BACKGROUND;
        let fg = config::COLOR_RESULT_VALUE;

        let mut y_pos = 20;
        let should_draw = self.last_position != self.position;

        if should_draw {
            self.draw_init(display).await;
        }

        for (index, label) in LABELS.iter().enumerate() {
            SMALL_FONT
                .render(
                    *label,
                    Point::new(10, y_pos),
                    VerticalPosition::Top,
                    if index == self.position {
                        FontColor::WithBackground { fg, bg }
                    } else {
                        FontColor::WithBackground { fg: bg, bg: fg }
                    },
                    display,
                )
                .unwrap();

            if index == self.position {
                y_pos += 40;
                match index {
                    0 => {
                        SMALL_FONT
                            .render(
                                "< START",
                                Point::new(10, y_pos),
                                VerticalPosition::Top,
                                FontColor::Transparent(config::COLOR_MENU_ACTION),
                                display,
                            )
                            .unwrap();
                        y_pos += 40;
                    }
                    1 => {
                        SMALL_FONT
                            .render(
                                "< START",
                                Point::new(10, y_pos),
                                VerticalPosition::Top,
                                FontColor::Transparent(config::COLOR_MENU_ACTION),
                                display,
                            )
                            .unwrap();
                        y_pos += 40;
                    }
                    2 => {
                        let mut x_pos = 10;
                        for (index, label) in SENSITIVITY_LABELS.iter().enumerate() {
                            let fg = config::COLOR_MENU_ACTION;
                            let rect = SMALL_FONT
                                .render(
                                    *label,
                                    Point::new(x_pos, y_pos),
                                    VerticalPosition::Top,
                                    if index == self.sensitivity as usize {
                                        FontColor::WithBackground { fg: bg, bg: fg }
                                    } else {
                                        FontColor::WithBackground { fg, bg }
                                    },
                                    display,
                                )
                                .unwrap();
                            x_pos = rect.bounding_box.unwrap().bottom_right().unwrap().x;
                        }
                        y_pos += 40;
                    }
                    3 => {
                        SMALL_FONT
                            .render(
                                "< DFU MODE",
                                Point::new(10, y_pos),
                                VerticalPosition::Top,
                                FontColor::Transparent(config::COLOR_MENU_ACTION),
                                display,
                            )
                            .unwrap();
                        y_pos += 40;
                    }
                    _ => (),
                }
            } else {
                y_pos += 20;
            }
        }
        self.last_position = self.position;
    }
}

impl MenuScreen<(), ()> {
    pub fn options_len() -> usize {
        LABELS.len()
    }
}

impl<DT: AppDrawTarget<E>, E: Debug> Default for MenuScreen<DT, E> {
    fn default() -> Self {
        Self {
            position: 0,
            sensitivity: 0,
            last_position: 999,
            _phantom: core::marker::PhantomData,
        }
    }
}
