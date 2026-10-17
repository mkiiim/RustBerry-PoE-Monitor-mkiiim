use embedded_graphics::prelude::Point;
use log::trace;

use crate::metrics::Metrics;
use crate::network::InterfaceAddr;
use super::config::{ComponentConfig, DisplayConfig, PositionValue};
use super::fonts::{font_by_name, text_width, Font};

/// The values a config can refer to by name, formatted for display.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayValues {
    /// Interface name, e.g. `eth0.20`
    pub interface: String,
    /// First display part of the interface name, e.g. `eth`
    pub interface_phys: String,
    /// Second display part of the interface name, e.g. `0.20`
    pub interface_numvlan: String,
    pub ip: String,
    pub ip_octets: [u8; 4],
    pub cpu_usage: String,
    pub cpu_temp: String,
    pub ram_usage: String,
    pub disk_usage: String,
}

impl DisplayValues {
    pub fn new(interface: &InterfaceAddr, metrics: &Metrics) -> Self {
        let (interface_phys, interface_numvlan) = interface.display_parts();
        DisplayValues {
            interface: interface.name.clone(),
            interface_phys,
            interface_numvlan,
            ip: interface.ip.to_string(),
            ip_octets: interface.ip.octets(),
            cpu_usage: format_reading(metrics.cpu_usage.into()),
            cpu_temp: metrics.cpu_temp.map_or_else(|| "--".to_string(), |t| format_reading(t.into())),
            ram_usage: format_reading(metrics.ram_usage),
            disk_usage: format_reading(metrics.disk_usage),
        }
    }

    /// Resolves a value name from the config; any other text is shown as is.
    fn resolve(&self, name: &str) -> String {
        match name {
            "interface_phys" => self.interface_phys.clone(),
            "interface_numvlan" => self.interface_numvlan.clone(),
            "ip_info.0" => self.interface.clone(),
            "ip_info.1" => self.ip.clone(),
            "ip_octets(0)" => self.ip_octets[0].to_string(),
            "ip_octets(1)" => self.ip_octets[1].to_string(),
            "ip_octets(2)" => self.ip_octets[2].to_string(),
            "ip_octets(3)" => self.ip_octets[3].to_string(),
            "cpu_usage" => self.cpu_usage.clone(),
            "cpu_temp" => self.cpu_temp.clone(),
            "ram_usage" => self.ram_usage.clone(),
            "disk_usage" => self.disk_usage.clone(),
            text => text.to_string(),
        }
    }
}

/// Formats a reading with one decimal, dropping it from 100 up so the value
/// stays four characters wide, e.g. "99.9" then "100".
fn format_reading(value: f64) -> String {
    if value >= 99.95 {
        format!("{:.0}", value)
    } else {
        format!("{:.1}", value)
    }
}

/// Text to draw with its left baseline at `position`.
pub struct TextRun {
    pub text: String,
    pub font: Font,
    pub position: Point,
}

/// Works out what to draw where. Each element's components (prefix, value,
/// suffix) are placed side by side, and the element is aligned as a whole.
pub fn layout(config: &DisplayConfig, values: &DisplayValues) -> Vec<TextRun> {
    let mut runs = Vec::new();

    for element in &config.elements {
        let parts: Vec<(String, Font)> = element.components.iter()
            .flat_map(|component| component_parts(component, values))
            .collect();
        let total_width: i32 = parts.iter().map(|(text, font)| text_width(text, font)).sum();

        let mut x = x_position(&element.position.x, total_width, config.width);
        let y = y_position(&element.position.y);
        trace!("Element {}: x={}, y={}, width={}", element.id, x, y, total_width);

        for (text, font) in parts {
            let width = text_width(&text, &font);
            runs.push(TextRun { text, font, position: Point::new(x, y) });
            x += width;
        }
    }
    runs
}

fn component_parts(component: &ComponentConfig, values: &DisplayValues) -> Vec<(String, Font)> {
    let mut parts = Vec::with_capacity(3);
    if let Some(prefix) = &component.prefix {
        parts.push((prefix.text.clone(), font_by_name(&prefix.font)));
    }
    parts.push((values.resolve(&component.value.text), font_by_name(&component.value.font)));
    if let Some(suffix) = &component.suffix {
        parts.push((suffix.text.clone(), font_by_name(&suffix.font)));
    }
    parts
}

fn x_position(x: &PositionValue, width: i32, display_width: i32) -> i32 {
    match x {
        PositionValue::Text(align) => match align.as_str() {
            "center" => (display_width - width) / 2,
            "right" => display_width - width,
            _ => 0,
        },
        PositionValue::Number(x) => *x,
        PositionValue::Relative { align, anchor } => match align.as_str() {
            "center" => anchor - width / 2,
            "right" => anchor - width,
            _ => *anchor,
        },
    }
}

fn y_position(y: &PositionValue) -> i32 {
    match y {
        PositionValue::Text(_) => 0,
        PositionValue::Number(y) => *y,
        PositionValue::Relative { anchor, .. } => *anchor,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> DisplayValues {
        DisplayValues {
            interface: "eth0.20".to_string(),
            interface_phys: "eth".to_string(),
            interface_numvlan: "0.20".to_string(),
            ip: "10.0.20.133".to_string(),
            ip_octets: [10, 0, 20, 133],
            cpu_usage: "12.5".to_string(),
            cpu_temp: "48.2".to_string(),
            ram_usage: "7.0".to_string(),
            disk_usage: "21.4".to_string(),
        }
    }

    fn config(json: &str) -> DisplayConfig {
        serde_json::from_str(json).unwrap()
    }

    fn summary(runs: &[TextRun]) -> Vec<(&str, i32, i32)> {
        runs.iter().map(|r| (r.text.as_str(), r.position.x, r.position.y)).collect()
    }

    #[test]
    fn builds_values_from_readings() {
        let interface = InterfaceAddr { name: "eth0.20".to_string(), ip: "10.0.20.133".parse().unwrap() };
        let metrics = Metrics { cpu_usage: 12.54, cpu_temp: Some(48.16), ram_usage: 7.0, disk_usage: 21.44 };
        assert_eq!(DisplayValues::new(&interface, &metrics), values());

        let no_temp = Metrics { cpu_temp: None, ..metrics };
        assert_eq!(DisplayValues::new(&interface, &no_temp).cpu_temp, "--");
    }

    #[test]
    fn readings_stay_four_characters_wide() {
        assert_eq!(format_reading(0.0), "0.0");
        assert_eq!(format_reading(99.94), "99.9");
        assert_eq!(format_reading(99.95), "100");
        assert_eq!(format_reading(100.0), "100");
    }

    #[test]
    fn full_cpu_fits_in_portrait() {
        let full = DisplayValues { cpu_usage: format_reading(100.0), ..values() };
        let runs = layout(&config(include_str!("../../etc_rustberry-poe-monitor/portrait.json")), &full);
        assert_eq!((runs[10].text.as_str(), runs[10].position.x), ("100", 6));
    }

    #[test]
    fn lays_out_portrait_config() {
        let runs = layout(&config(include_str!("../../etc_rustberry-poe-monitor/portrait.json")), &values());
        let runs = summary(&runs);
        assert_eq!(runs.len(), 21);
        assert_eq!(runs[0], ("eth", 0, 8));                // left
        assert_eq!(runs[1], ("0.20", 0, 16));
        assert_eq!(runs[2], ("10", 14, 26));               // left of anchor 14
        assert_eq!(runs[3], (".", 8, 34));                 // right of anchor 14
        assert_eq!(runs[8], ("133", 14, 50));
        assert_eq!(runs[9], ("CPU", 17, 63));              // right edge, 3 * 5px
        assert_eq!(runs[10], ("12.5", 0, 72));             // right of anchor 24, 4 * 6px
        assert_eq!(runs[11], ("%", 26, 72));
        assert_eq!(runs[19], ("21.4", 0, 123));
    }

    #[test]
    fn places_components_side_by_side() {
        let json = r#"{"orientation": "landscape", "width": 128, "height": 32, "elements": [
            {"id": "temp", "position": {"x": "center", "y": 10}, "components": [
                {"prefix": {"text": "T: ", "font": "FONT_5X8"},
                 "value": {"text": "cpu_temp", "font": "FONT_6X12"},
                 "suffix": {"text": "C", "font": "FONT_5X8"}}]}]}"#;
        let runs = layout(&config(json), &values());
        // 15 + 24 + 5 = 44px wide, centred: (128 - 44) / 2 = 42
        assert_eq!(summary(&runs), vec![("T: ", 42, 10), ("48.2", 57, 10), ("C", 81, 10)]);
    }

    #[test]
    fn aligns_elements() {
        assert_eq!(x_position(&PositionValue::Text("left".into()), 20, 128), 0);
        assert_eq!(x_position(&PositionValue::Text("right".into()), 20, 128), 108);
        assert_eq!(x_position(&PositionValue::Text("center".into()), 20, 128), 54);
        assert_eq!(x_position(&PositionValue::Number(7), 20, 128), 7);
        assert_eq!(x_position(&PositionValue::Relative { align: "center".into(), anchor: 64 }, 20, 128), 54);
        assert_eq!(y_position(&PositionValue::Text("incrementing".into())), 0);
        assert_eq!(y_position(&PositionValue::Relative { align: "left".into(), anchor: 9 }), 9);
    }

    #[test]
    fn literal_text_is_shown_as_is() {
        assert_eq!(values().resolve("CPU"), "CPU");
        assert_eq!(values().resolve("ip_octets(3)"), "133");
        assert_eq!(values().resolve("ip_info.0"), "eth0.20");
    }
}
