use std::process::Command;
use super::config::{Align, ComponentConfig, DisplayConfig, ElementConfig, Orientation,
                    PositionConfig, ValueConfig, XPosition};
use super::fonts::FontName;

/// Layout shown when the config file can't be loaded: the hostname and
/// "CONFIG ERROR", so the problem is visible when the screen is woken.
pub fn config_error_display_config() -> DisplayConfig {
    error_display_config(&get_system_hostname())
}

fn error_display_config(hostname: &str) -> DisplayConfig {
    DisplayConfig {
        orientation: Orientation::Landscape,
        width: 128,
        height: 32,
        elements: vec![
            centred_line("hostname", 8, hostname, FontName::Font6x12),
            centred_line("config_error", 22, "CONFIG ERROR", FontName::PcSenior8),
        ],
    }
}

fn centred_line(id: &str, y: i32, text: &str, font: FontName) -> ElementConfig {
    ElementConfig {
        id: id.to_string(),
        position: PositionConfig { x: XPosition::Screen(Align::Center), y },
        components: vec![ComponentConfig {
            value: ValueConfig { text: text.to_string(), font },
            prefix: None,
            suffix: None,
        }],
    }
}

fn get_system_hostname() -> String {
    match Command::new("hostname").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        },
        _ => "RustBerry".to_string(),  // Default if hostname command fails
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::layout::{layout, DisplayValues};

    #[test]
    fn shows_hostname_and_error() {
        let values = DisplayValues {
            interface: String::new(), interface_phys: String::new(), interface_numvlan: String::new(),
            ip: String::new(), ip_octets: [0; 4], cpu_usage: String::new(), cpu_temp: String::new(),
            ram_usage: String::new(), disk_usage: String::new(),
        };
        let runs = layout(&error_display_config("rpi-pihole2"), &values);
        let runs: Vec<_> = runs.iter().map(|r| (r.text.as_str(), r.position.x, r.position.y)).collect();
        // 11 * 6px and 12 * 8px wide, centred on 128px
        assert_eq!(runs, vec![("rpi-pihole2", 31, 8), ("CONFIG ERROR", 16, 22)]);
    }
}
