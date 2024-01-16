use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};

const fn from_888(r: u8, g: u8, b: u8) -> Rgb565 {
    Rgb565::new(r >> 3, g >> 2, b >> 3)
}

macro_rules! normal_and_inactive {
    ($na:ident, $nb:ident, $r:expr, $g:expr, $b:expr) => {
        pub const $na: Rgb565 = from_888($r, $g, $b);
        pub const $nb: Rgb565 = from_888($r / 3, $g / 3, $b / 3);
    };
}

pub const COLOR_BACKGROUND: Rgb565 = Rgb565::BLACK;
pub const COLOR_RESULT_VALUE: Rgb565 = Rgb565::WHITE;
pub const COLOR_RESULT_VALUE_INACTIVE: Rgb565 = Rgb565::new(4, 8, 4);

normal_and_inactive!(COLOR_RESULT_GOOD, COLOR_RESULT_GOOD_INACTIVE, 152, 251, 152);
normal_and_inactive!(COLOR_RESULT_FAIR, COLOR_RESULT_FAIR_INACTIVE, 255, 69, 0);
normal_and_inactive!(COLOR_RESULT_BAD, COLOR_RESULT_BAD_INACTIVE, 255, 0, 0);

pub const COLOR_LEVEL: Rgb565 = Rgb565::CSS_PALE_GREEN;
pub const COLOR_NOISE: Rgb565 = Rgb565::RED;
pub const COLOR_CALIBRATION: Rgb565 = Rgb565::YELLOW;
pub const COLOR_TRIGGER_HIGH: Rgb565 = Rgb565::CSS_TURQUOISE;
pub const COLOR_TRIGGER_LOW: Rgb565 = Rgb565::CSS_DARK_ORANGE;

pub const COLOR_CHART_1: Rgb565 = Rgb565::new(7, 0, 0);
pub const COLOR_CHART_2: Rgb565 = Rgb565::CSS_DARK_RED;
pub const COLOR_CHART_3: Rgb565 = Rgb565::RED;

pub const COLOR_NEAREST_SPEED: Rgb565 = Rgb565::CYAN;

pub const COLOR_RULER: Rgb565 = Rgb565::CSS_PALE_GREEN;

pub const COLOR_MENU_ACTION: Rgb565 = Rgb565::CSS_PALE_GREEN;
