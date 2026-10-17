use embedded_graphics::prelude::Point;
use log::trace;

use crate::metrics::Metrics;
use crate::network::InterfaceAddr;
use super::config::{Align, ComponentConfig, DisplayConfig, XPosition};
use super::fonts::{text_width, Font};

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

    /// Fixed values for `--render --sample`, so renders can be compared.
    pub fn sample() -> Self {
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

    fn get(&self, name: ValueName) -> String {
        match name {
            ValueName::InterfacePhys => self.interface_phys.clone(),
            ValueName::InterfaceNumVlan => self.interface_numvlan.clone(),
            ValueName::Interface => self.interface.clone(),
            ValueName::Ip => self.ip.clone(),
            ValueName::IpOctet(i) => self.ip_octets[i].to_string(),
            ValueName::CpuUsage => self.cpu_usage.clone(),
            ValueName::CpuTemp => self.cpu_temp.clone(),
            ValueName::RamUsage => self.ram_usage.clone(),
            ValueName::DiskUsage => self.disk_usage.clone(),
        }
    }

    /// Resolves a value name from the config; any other text is shown as is.
    fn resolve(&self, text: &str) -> String {
        ValueName::parse(text).map_or_else(|| text.to_string(), |name| self.get(name))
    }
}

/// The value names a config can use in place of literal text.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ValueName {
    InterfacePhys,
    InterfaceNumVlan,
    Interface,
    Ip,
    IpOctet(usize),
    CpuUsage,
    CpuTemp,
    RamUsage,
    DiskUsage,
}

impl ValueName {
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text {
            "interface_phys" => ValueName::InterfacePhys,
            "interface_numvlan" => ValueName::InterfaceNumVlan,
            "ip_info.0" => ValueName::Interface,
            "ip_info.1" => ValueName::Ip,
            "ip_octets(0)" => ValueName::IpOctet(0),
            "ip_octets(1)" => ValueName::IpOctet(1),
            "ip_octets(2)" => ValueName::IpOctet(2),
            "ip_octets(3)" => ValueName::IpOctet(3),
            "cpu_usage" => ValueName::CpuUsage,
            "cpu_temp" => ValueName::CpuTemp,
            "ram_usage" => ValueName::RamUsage,
            "disk_usage" => ValueName::DiskUsage,
            _ => return None,
        })
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
    pub width: i32,
}

/// Works out what to draw where. Each element's components (prefix, value,
/// suffix) are placed side by side, and the element is aligned as a whole.
/// Aligned text that would start left of the screen starts at the left edge
/// instead, so its beginning stays readable.
pub fn layout(config: &DisplayConfig, values: &DisplayValues) -> Vec<TextRun> {
    let mut runs = Vec::new();

    for element in &config.elements {
        let parts: Vec<(String, Font)> = element.components.iter()
            .flat_map(|component| component_parts(component, values))
            .collect();
        let total_width: i32 = parts.iter().map(|(text, font)| text_width(text, font)).sum();

        let mut x = x_position(&element.position.x, total_width, config.width);
        if x < 0 && !matches!(element.position.x, XPosition::Pixel(_)) {
            x = 0;
        }
        let y = element.position.y;
        trace!("Element {}: x={}, y={}, width={}", element.id, x, y, total_width);

        for (text, font) in parts {
            let width = text_width(&text, &font);
            runs.push(TextRun { text, font, position: Point::new(x, y), width });
            x += width;
        }
    }
    runs
}

/// The first run that doesn't fit within `display_width`, if any.
pub fn off_screen(runs: &[TextRun], display_width: i32) -> Option<&TextRun> {
    runs.iter().find(|run| run.position.x < 0 || run.position.x + run.width > display_width)
}

fn component_parts(component: &ComponentConfig, values: &DisplayValues) -> Vec<(String, Font)> {
    let mut parts = Vec::with_capacity(3);
    if let Some(prefix) = &component.prefix {
        parts.push((prefix.text.clone(), prefix.font.style()));
    }
    parts.push((values.resolve(&component.value.text), component.value.font.style()));
    if let Some(suffix) = &component.suffix {
        parts.push((suffix.text.clone(), suffix.font.style()));
    }
    parts
}

fn x_position(x: &XPosition, width: i32, display_width: i32) -> i32 {
    match *x {
        XPosition::Screen(align) => align_at(align, 0, display_width, width),
        XPosition::Pixel(x) => x,
        XPosition::Anchor { align, anchor } => align_at(align, anchor, 0, width),
    }
}

/// Start of text `width` wide, placed relative to `left` (left-aligned),
/// `left + span / 2` (centred) or `left + span` (right-aligned).
fn align_at(align: Align, left: i32, span: i32, width: i32) -> i32 {
    match align {
        Align::Left => left,
        Align::Center => left + (span - width) / 2,
        Align::Right => left + span - width,
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
        DisplayConfig::parse(json).unwrap()
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
    fn keeps_start_of_wide_text_on_screen() {
        let wide = DisplayValues {
            interface: "eth0.99".to_string(), ip_octets: [192, 168, 100, 200], ..values()
        };
        let config = config(include_str!("../../etc_rustberry-poe-monitor/landscape.json"));
        let runs = layout(&config, &wide);
        // "eth0.99: " is 9 * 5px and "192.168.100.200" 15 * 6px, 135px in all
        assert_eq!((runs[0].text.as_str(), runs[0].position.x), ("eth0.99", 0));
        let last = off_screen(&runs, config.width).unwrap();
        assert_eq!((last.text.as_str(), last.position.x), ("200", 117));

        // The shipped landscape layout puts "%DISK" at 103, so "DISK" ends at 129
        let runs = layout(&config, &values());
        let disk = off_screen(&runs, config.width).unwrap();
        assert_eq!((disk.text.as_str(), disk.position.x + disk.width), ("DISK", 129));

        let portrait = self::config(include_str!("../../etc_rustberry-poe-monitor/portrait.json"));
        assert!(off_screen(&layout(&portrait, &values()), portrait.width).is_none());
    }

    #[test]
    fn aligns_elements() {
        assert_eq!(x_position(&XPosition::Screen(Align::Left), 20, 128), 0);
        assert_eq!(x_position(&XPosition::Screen(Align::Right), 20, 128), 108);
        assert_eq!(x_position(&XPosition::Screen(Align::Center), 20, 128), 54);
        assert_eq!(x_position(&XPosition::Pixel(7), 20, 128), 7);
        assert_eq!(x_position(&XPosition::Anchor { align: Align::Left, anchor: 64 }, 20, 128), 64);
        assert_eq!(x_position(&XPosition::Anchor { align: Align::Center, anchor: 64 }, 20, 128), 54);
        assert_eq!(x_position(&XPosition::Anchor { align: Align::Right, anchor: 64 }, 20, 128), 44);
    }

    #[test]
    fn literal_text_is_shown_as_is() {
        assert_eq!(values().resolve("CPU"), "CPU");
        assert_eq!(values().resolve("ip_octets(3)"), "133");
        assert_eq!(values().resolve("ip_info.0"), "eth0.20");
    }
}
