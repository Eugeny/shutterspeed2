use u8g2_fonts::fonts::{
    u8g2_font_micro_mr, u8g2_font_profont10_mr, u8g2_font_profont17_mr, u8g2_font_spleen16x32_mn,
    u8g2_font_t0_15b_mr,
};
use u8g2_fonts::FontRenderer;

pub type TinyFont = u8g2_font_profont10_mr;
pub type TinierFont = u8g2_font_micro_mr;
pub const SMALL_FONT: FontRenderer = FontRenderer::new::<u8g2_font_profont17_mr>();
pub const TINY_FONT: FontRenderer = FontRenderer::new::<TinyFont>();
pub const TINIER_FONT: FontRenderer = FontRenderer::new::<TinierFont>();
pub const LARGE_DIGIT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_spleen16x32_mn>();

pub const ALT_FONT: FontRenderer = FontRenderer::new::<u8g2_font_t0_15b_mr>();
