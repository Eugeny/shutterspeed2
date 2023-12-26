use u8g2_fonts::FontRenderer;
use u8g2_fonts::fonts::{u8g2_font_profont29_mr, u8g2_font_profont17_mr, u8g2_font_spleen32x64_mn};


pub type TinyFont = u8g2_font_profont17_mr;
// const TEXT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen16x32_me>();
pub const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont29_mr>();
pub const TINY_FONT: FontRenderer = FontRenderer::new::<TinyFont>();
pub const LARGE_DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen32x64_mn>();
