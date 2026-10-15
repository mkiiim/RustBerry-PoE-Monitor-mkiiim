use embedded_graphics::{
    image::ImageRaw,
    mono_font::{ascii, MonoTextStyleBuilder, MonoFont, MonoTextStyle, DecorationDimensions, mapping::StrGlyphMapping},
    pixelcolor::BinaryColor,
    prelude::*
};
use profont::{PROFONT_12_POINT, PROFONT_9_POINT};

pub type Font = MonoTextStyle<'static, BinaryColor>;

pub const PROFONT12: Font = MonoTextStyleBuilder::new()
    .font(&PROFONT_12_POINT)
    .text_color(BinaryColor::On)
    .build();

pub const PROFONT9: Font = MonoTextStyleBuilder::new()
    .font(&PROFONT_9_POINT)
    .text_color(BinaryColor::On)
    .build();

pub const FONT_6X12: Font = MonoTextStyleBuilder::new()
    .font(&ascii::FONT_6X12)
    .text_color(BinaryColor::On)
    .build();

pub const FONT_5X8: Font = MonoTextStyleBuilder::new()
    .font(&ascii::FONT_5X8)
    .text_color(BinaryColor::On)
    .build();

pub const GLYPH_MAPPING: StrGlyphMapping = StrGlyphMapping::new(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~°", '?' as usize - ' ' as usize);

pub const PCSENIOR8: MonoFont = MonoFont {
    image: ImageRaw::new(
        include_bytes!("../../data/pcsenior.raw"),
        128,
    ),
    character_size: Size::new(8, 10),
    character_spacing: 0,
    baseline: 7,
    underline: DecorationDimensions::new(9, 1),
    strikethrough: DecorationDimensions::new(10 / 2, 1),
    glyph_mapping: &GLYPH_MAPPING,
};

pub const PCSENIOR8_STYLE: Font = MonoTextStyleBuilder::new()
    .font(&PCSENIOR8)
    .text_color(BinaryColor::On)
    .build();

/// Looks up a font by the name used in the JSON config. Unknown names fall back to `FONT_5X8`.
pub fn font_by_name(name: &str) -> Font {
    match name {
        "FONT_5X8" => FONT_5X8,
        "FONT_6X12" => FONT_6X12,
        "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
        "PROFONT12" => PROFONT12,
        "PROFONT9" => PROFONT9,
        _ => FONT_5X8,
    }
}

/// Horizontal advance of one character, including the font's spacing.
pub fn char_width(font: &Font) -> i32 {
    font.font.character_size.width as i32 + font.font.character_spacing as i32
}

/// Width of `text` in pixels. Uses the byte length, so a non-ASCII glyph
/// such as '°' is counted as two characters.
pub fn text_width(text: &str, font: &Font) -> i32 {
    text.len() as i32 * char_width(font)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_fonts_by_name() {
        assert_eq!(char_width(&font_by_name("FONT_6X12")), 6);
        assert_eq!(char_width(&font_by_name("FONT_5X8")), 5);
        assert_eq!(char_width(&font_by_name("PCSENIOR8_STYLE")), 8);
        assert_eq!(char_width(&font_by_name("no-such-font")), 5);
    }

    #[test]
    fn measures_text() {
        assert_eq!(text_width("12.5", &FONT_6X12), 24);
        assert_eq!(text_width("", &FONT_6X12), 0);
    }
}
