use embedded_graphics::geometry::{Point, Size};
use embedded_graphics::pixelcolor::{Rgb565, RgbColor};
use embedded_graphics::primitives::Rectangle;
use embedded_graphics::Drawable;
use embedded_text::style::{HeightMode, TextBoxStyleBuilder};
use embedded_text::TextBox;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::U8g2TextStyle;

use crate::fonts::{TinyFont, SMALL_FONT, TINY_FONT};
use crate::primitives::Cross;
use crate::AppDrawTarget;

pub fn draw_panic_screen<D: AppDrawTarget<E>, E>(display: &mut D, message: &str) {
    let width = display.bounding_box().size.width;
    let height = display.bounding_box().size.height;

    let _ = display.fill_solid(&display.bounding_box(), Rgb565::RED);

    for d in [-1, 0, 1] {
        let _ =
            Cross::new(Point::new(width as i32 / 2 + d * 40, 50), 15, Rgb565::BLACK).draw(display);
    }

    let _ = TINY_FONT.render_aligned(
        env!("CARGO_PKG_VERSION"),
        Point::new(width as i32 / 2, 80),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            fg: Rgb565::BLACK,
            bg: Rgb565::RED,
        },
        display,
    );

    let _ = SMALL_FONT.render_aligned(
        " FATAL ERROR ",
        Point::new(width as i32 / 2, 100),
        VerticalPosition::Top,
        HorizontalAlignment::Center,
        FontColor::WithBackground {
            fg: Rgb565::RED,
            bg: Rgb565::BLACK,
        },
        display,
    );

    let character_style = U8g2TextStyle::new(TinyFont {}, Rgb565::BLACK);

    let textbox_style = TextBoxStyleBuilder::new()
        .height_mode(HeightMode::FitToText)
        .alignment(embedded_text::alignment::HorizontalAlignment::Center)
        .build();

    let origin = Point::new(10, 150);
    let _ = TextBox::with_textbox_style(
        message,
        Rectangle::new(origin, Size::new(width - 20, height - origin.y as u32)),
        character_style,
        textbox_style,
    )
    .draw(display);
}
