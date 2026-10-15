use std::fs;
use ssd1306::prelude::DisplayRotation;
use serde::{Deserialize, Serialize};
use log::info;

use super::DisplayError;

#[derive(Deserialize, Serialize, Debug)]
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
#[derive(Deserialize)]
pub struct DisplayConfig {
    pub orientation: Orientation,
    pub width: i32,
    pub height: i32,
    pub elements: Vec<ElementConfig>,
}

impl DisplayConfig {
    pub fn load(path: &str) -> Result<Self, DisplayError> {
        info!("Loading config file from: {}", path);
        let config: DisplayConfig = serde_json::from_str(&fs::read_to_string(path)?)?;
        info!("Configuration loaded with orientation: {:?}", config.orientation);
        Ok(config)
    }
}

/// A line of text made of components drawn side by side at one position.
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
        anchor: i32
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

/// `text` is either a value name (e.g. `cpu_temp`, `ip_octets(0)`) or literal text.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_shipped_configs() {
        let portrait: DisplayConfig = serde_json::from_str(include_str!("../../etc_rustberry-poe-monitor/portrait.json")).unwrap();
        assert!(matches!(portrait.orientation, Orientation::Portrait));
        assert_eq!((portrait.width, portrait.height), (32, 128));

        let landscape: DisplayConfig = serde_json::from_str(include_str!("../../etc_rustberry-poe-monitor/landscape.json")).unwrap();
        assert!(matches!(landscape.orientation, Orientation::Landscape));
        assert_eq!((landscape.width, landscape.height), (128, 32));
    }
}
