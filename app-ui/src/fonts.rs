use u8g2_fonts::fonts::{
    u8g2_font_profont15_mr, u8g2_font_profont17_mr, u8g2_font_profont29_mr,
    u8g2_font_spleen32x64_mn,
};
use u8g2_fonts::FontRenderer;

pub type TinyFont = u8g2_font_profont17_mr;
pub type TinierFont = u8g2_font_profont15_mr;
pub const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont29_mr>();
pub const TINY_FONT: FontRenderer = FontRenderer::new::<TinyFont>();
pub const TINIER_FONT: FontRenderer = FontRenderer::new::<TinierFont>();
pub const LARGE_DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen32x64_mn>();
