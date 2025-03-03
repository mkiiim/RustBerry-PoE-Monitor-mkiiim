use linux_embedded_hal::I2cdev;
use ssd1306::{prelude::*, Ssd1306, mode::BufferedGraphicsMode};
use embedded_graphics::{
    image::ImageRaw,
    mono_font::{ascii, MonoTextStyleBuilder, MonoFont, MonoTextStyle, DecorationDimensions, mapping::StrGlyphMapping},
    pixelcolor::BinaryColor,
    prelude::*
};
use profont::{PROFONT_12_POINT, PROFONT_9_POINT};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct DisplayConfig {
    pub orientations: Orientations,
}

#[derive(Deserialize)]
pub struct Orientations {
    pub landscape: OrientationConfig,
    pub portrait: OrientationConfig,
}

#[derive(Deserialize)]
pub struct OrientationConfig {
    pub width: i32,
    pub height: i32,
    pub elements: Vec<ElementConfig>,
}

#[derive(Deserialize)]
pub struct ElementConfig {
    pub id: String,
    pub position: PositionConfig,
    pub components: Vec<ComponentConfig>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum PositionValue {
    Number(i32),
    Text(String),
    Relative {
        align: String,
        reference: i32
    }
}

#[derive(Deserialize)]
pub struct PositionConfig {
    pub x: PositionValue,
    pub y: PositionValue,
}

#[derive(Deserialize)]
pub struct ComponentConfig {
    pub value: ValueConfig,
    pub prefix: Option<PrefixSuffixConfig>,
    pub suffix: Option<PrefixSuffixConfig>,
}

#[derive(Deserialize)]
pub struct ValueConfig {
    pub text: String,
    pub font: String,
}

#[derive(Deserialize)]
pub struct PrefixSuffixConfig {
    pub text: String,
    pub font: String,
}




pub type Display = Ssd1306<I2CInterface<I2cdev>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

pub const PROFONT12: MonoTextStyle<'_, BinaryColor> = MonoTextStyleBuilder::new()
.font(&PROFONT_12_POINT)
.text_color(BinaryColor::On)
.build();

pub const PROFONT9: MonoTextStyle<'_, BinaryColor> = MonoTextStyleBuilder::new()
.font(&PROFONT_9_POINT)
.text_color(BinaryColor::On)
.build();

pub const FONT_6X12: MonoTextStyle<'_, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&ascii::FONT_6X12)
    .text_color(BinaryColor::On)
    .build();

pub const FONT_5X8: MonoTextStyle<'_, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&ascii::FONT_5X8)
    .text_color(BinaryColor::On)
    .build();

pub const GLYPH_MAPPING: StrGlyphMapping = StrGlyphMapping::new(" !\"#$%&'()*+,-./0123456789:;<=>?@ABCDEFGHIJKLMNOPQRSTUVWXYZ[\\]^_`abcdefghijklmnopqrstuvwxyz{|}~°", '?' as usize - ' ' as usize);

pub const PCSENIOR8: MonoFont = MonoFont {
    image: ImageRaw::new(
        include_bytes!("../data/pcsenior.raw"),
        128,
    ),
    character_size: Size::new(8, 10),
    character_spacing: 0,
    baseline: 7,
    underline: DecorationDimensions::new(9, 1),
    strikethrough: DecorationDimensions::new(10 / 2, 1),
    glyph_mapping: &GLYPH_MAPPING,
};

pub const PCSENIOR8_STYLE: MonoTextStyle<'_, BinaryColor> = MonoTextStyleBuilder::new()
    .font(&PCSENIOR8)
    .text_color(BinaryColor::On)
    .build();
