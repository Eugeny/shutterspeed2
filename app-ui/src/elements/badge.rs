use core::fmt::Debug;

use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::Rgb565;
#[cfg(feature = "cortex-m")]
use fugit::ExtU32;
#[cfg(feature = "cortex-m")]
use rtic_monotonics::systick::Systick;
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};

use crate::fonts::SMALL_FONT;
use crate::AppDrawTarget;

pub async fn draw_badge<D: AppDrawTarget<E>, E: Debug>(
    display: &mut D,
    point: Point,
    text: &str,
    fg: Rgb565,
    bg: Rgb565,
) {
    SMALL_FONT
        .render_aligned(
            text,
            point,
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground { fg: bg, bg: fg },
            display,
        )
        .unwrap();

    display.hint_refresh();
    #[cfg(feature = "cortex-m")]
    Systick::delay(50.millis()).await;
    #[cfg(feature = "std")]
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    SMALL_FONT
        .render_aligned(
            text,
            point,
            VerticalPosition::Top,
            HorizontalAlignment::Center,
            FontColor::WithBackground { fg, bg },
            display,
        )
        .unwrap();
}
