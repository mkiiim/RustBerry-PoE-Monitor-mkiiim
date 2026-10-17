use embedded_graphics::{
    image::ImageRaw,
    mono_font::{ascii, MonoTextStyleBuilder, MonoFont, MonoTextStyle, DecorationDimensions, mapping::StrGlyphMapping},
    pixelcolor::BinaryColor,
    prelude::*
};
use profont::{PROFONT_12_POINT, PROFONT_9_POINT};
use serde::Deserialize;

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

/// Font names as used in the JSON config.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum FontName {
    #[serde(rename = "FONT_5X8")]
    Font5x8,
    #[serde(rename = "FONT_6X12")]
    Font6x12,
    #[serde(rename = "PCSENIOR8_STYLE")]
    PcSenior8,
    #[serde(rename = "PROFONT12")]
    ProFont12,
    #[serde(rename = "PROFONT9")]
    ProFont9,
}

impl FontName {
    pub fn style(self) -> Font {
        match self {
            FontName::Font5x8 => FONT_5X8,
            FontName::Font6x12 => FONT_6X12,
            FontName::PcSenior8 => PCSENIOR8_STYLE,
            FontName::ProFont12 => PROFONT12,
            FontName::ProFont9 => PROFONT9,
        }
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
        let width = |name: &str| char_width(&serde_json::from_str::<FontName>(name).unwrap().style());
        assert_eq!(width(r#""FONT_6X12""#), 6);
        assert_eq!(width(r#""FONT_5X8""#), 5);
        assert_eq!(width(r#""PCSENIOR8_STYLE""#), 8);
        assert_eq!(width(r#""PROFONT9""#), 6);
        assert!(serde_json::from_str::<FontName>(r#""no-such-font""#).is_err());
    }

    #[test]
    fn measures_text() {
        assert_eq!(text_width("12.5", &FONT_6X12), 24);
        assert_eq!(text_width("", &FONT_6X12), 0);
    }
}
