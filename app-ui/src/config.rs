use embedded_graphics::pixelcolor::{Rgb565, RgbColor, WebColors};

pub const COLOR_BACKGROUND: Rgb565 = Rgb565::BLACK;
pub const COLOR_RESULT_VALUE: Rgb565 = Rgb565::WHITE;
pub const COLOR_RESULT_VALUE_INACTIVE: Rgb565 = Rgb565::new(4, 8, 4);
pub const COLOR_RESULT_GOOD: Rgb565 = Rgb565::CSS_PALE_GREEN;
pub const COLOR_RESULT_FAIR: Rgb565 = Rgb565::CSS_ORANGE_RED;
pub const COLOR_RESULT_BAD: Rgb565 = Rgb565::CSS_RED;

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
