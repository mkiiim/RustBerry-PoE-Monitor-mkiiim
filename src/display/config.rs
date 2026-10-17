use std::fs;
use ssd1306::prelude::DisplayRotation;
use serde::Deserialize;
use log::info;

use super::DisplayError;
use super::fonts::FontName;
use super::layout::ValueName;

#[derive(Deserialize, Debug)]
pub enum Orientation {
    #[serde(rename = "landscape")]
    Landscape,
    #[serde(rename = "portrait")]
    Portrait,
}

impl Orientation {
    pub fn to_display_rotation(&self) -> DisplayRotation {
        match self {
            Orientation::Portrait => DisplayRotation::Rotate90,
            Orientation::Landscape => DisplayRotation::Rotate0,
        }
    }
}

/// Display layout loaded from a JSON file such as `/etc/rustberry-poe-monitor/portrait.json`.
/// Unknown fields, fonts and alignments are rejected when the file is parsed.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayConfig {
    pub orientation: Orientation,
    pub width: i32,
    pub height: i32,
    pub elements: Vec<ElementConfig>,
}

impl DisplayConfig {
    pub fn load(path: &str) -> Result<Self, DisplayError> {
        info!("Loading config file from: {}", path);
        let config = Self::parse(&fs::read_to_string(path)?)?;
        info!("Configuration loaded with orientation: {:?}", config.orientation);
        Ok(config)
    }

    pub fn parse(json: &str) -> Result<Self, DisplayError> {
        let config: DisplayConfig = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Rejects value text that looks like a value name but isn't one, such as
    /// `cpu_tmep`, which would otherwise be shown on screen as is.
    fn validate(&self) -> Result<(), DisplayError> {
        for element in &self.elements {
            for component in &element.components {
                let text = &component.value.text;
                if looks_like_value_name(text) && ValueName::parse(text).is_none() {
                    return Err(DisplayError::Config(format!(
                        "element \"{}\": unknown value \"{}\"", element.id, text)));
                }
            }
        }
        Ok(())
    }
}

/// Lowercase text with an underscore, e.g. `cpu_temp` or `ip_octets(0)`.
/// Labels such as `CPU`, `%` or `.` don't match.
fn looks_like_value_name(text: &str) -> bool {
    text.contains('_')
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_.()".contains(c))
}

/// A line of text made of components drawn side by side at one position.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ElementConfig {
    pub id: String,
    pub position: PositionConfig,
    pub components: Vec<ComponentConfig>,
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Horizontal position: aligned to the screen (`"center"`), at a pixel
/// (`7`), or aligned to an anchor pixel (`{"align": "right", "anchor": 38}`).
#[derive(Deserialize, Debug, PartialEq)]
#[serde(try_from = "serde_json::Value")]
pub enum XPosition {
    Screen(Align),
    Pixel(i32),
    Anchor { align: Align, anchor: i32 },
}

impl TryFrom<serde_json::Value> for XPosition {
    type Error = String;

    fn try_from(value: serde_json::Value) -> Result<Self, Self::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Anchor {
            align: Align,
            anchor: i32,
        }

        let position = if value.is_string() {
            serde_json::from_value(value).map(XPosition::Screen)
        } else if value.is_number() {
            serde_json::from_value(value).map(XPosition::Pixel)
        } else {
            serde_json::from_value(value).map(|a: Anchor| XPosition::Anchor { align: a.align, anchor: a.anchor })
        };
        position.map_err(|e| format!(
            "x must be \"left\", \"center\", \"right\", a number or {{\"align\", \"anchor\"}}: {}", e))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PositionConfig {
    pub x: XPosition,
    /// Baseline in pixels from the top
    pub y: i32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentConfig {
    pub value: ValueConfig,
    pub prefix: Option<TextConfig>,
    pub suffix: Option<TextConfig>,
}

/// `text` is either a value name (e.g. `cpu_temp`, `ip_octets(0)`) or literal text.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ValueConfig {
    pub text: String,
    pub font: FontName,
}

/// Literal text, used for prefixes and suffixes.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TextConfig {
    pub text: String,
    pub font: FontName,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ELEMENT: &str = r#"{"id": "temp", "position": {"x": "center", "y": 10},
        "components": [{"value": {"text": "cpu_temp", "font": "FONT_6X12"}}]}"#;

    fn with_element(element: &str) -> String {
        format!(r#"{{"orientation": "landscape", "width": 128, "height": 32, "elements": [{}]}}"#, element)
    }

    fn error(element: &str) -> String {
        match DisplayConfig::parse(&with_element(element)) {
            Ok(_) => panic!("accepted: {}", element),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn parses_shipped_configs() {
        let portrait = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/portrait.json")).unwrap();
        assert!(matches!(portrait.orientation, Orientation::Portrait));
        assert_eq!((portrait.width, portrait.height), (32, 128));

        let landscape = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/landscape.json")).unwrap();
        assert!(matches!(landscape.orientation, Orientation::Landscape));
        assert_eq!((landscape.width, landscape.height), (128, 32));
    }

    #[test]
    fn parses_positions() {
        let x = |json: &str| serde_json::from_str::<XPosition>(json);
        assert_eq!(x(r#""right""#).unwrap(), XPosition::Screen(Align::Right));
        assert_eq!(x("7").unwrap(), XPosition::Pixel(7));
        assert_eq!(x(r#"{"align": "left", "anchor": 14}"#).unwrap(), XPosition::Anchor { align: Align::Left, anchor: 14 });
        assert!(x(r#""centre""#).is_err());
        assert!(x(r#"{"align": "left"}"#).is_err());
        assert!(x(r#"{"align": "left", "anchor": 14, "offset": 2}"#).is_err());
        assert!(DisplayConfig::parse(&with_element(ELEMENT)).is_ok());
    }

    #[test]
    fn rejects_unknown_font() {
        let e = error(&ELEMENT.replace("FONT_6X12", "FONT_6x12"));
        assert!(e.contains("unknown variant `FONT_6x12`"), "{}", e);
        assert!(e.contains("line 2"), "{}", e);
    }

    #[test]
    fn rejects_unknown_alignment() {
        let e = error(&ELEMENT.replace(r#""center""#, r#""centre""#));
        assert!(e.contains("x must be"), "{}", e);
    }

    #[test]
    fn rejects_text_y() {
        assert!(error(&ELEMENT.replace(r#""y": 10"#, r#""y": "top""#)).contains("expected i32"));
    }

    #[test]
    fn rejects_unknown_fields() {
        let e = error(&ELEMENT.replace(r#""font""#, r#""fnt""#));
        assert!(e.contains("unknown field `fnt`"), "{}", e);
    }

    #[test]
    fn rejects_misspelled_value_names() {
        assert_eq!(error(&ELEMENT.replace("cpu_temp", "cpu_tmep")),
                   "Config error: element \"temp\": unknown value \"cpu_tmep\"");
        assert!(error(&ELEMENT.replace("cpu_temp", "ip_octets(4)")).contains("ip_octets(4)"));
    }

    #[test]
    fn accepts_labels() {
        for label in ["CPU", "%", ".", "o", ": ", "°", "Hello World!", "rpi-pidev"] {
            assert!(!looks_like_value_name(label), "{}", label);
        }
        assert!(looks_like_value_name("ip_info.0"));
        assert!(looks_like_value_name("ip_octets(0)"));
    }
}
