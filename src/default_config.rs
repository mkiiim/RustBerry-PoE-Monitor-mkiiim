use std::process::Command;
use crate::display_types::{DisplayConfig, Orientation, ElementConfig, PositionConfig, 
                           PositionValue, ComponentConfig, ValueConfig, PrefixSuffixConfig};

pub fn get_default_display_config() -> DisplayConfig {
    // Get hostname for the first line
    let hostname = get_system_hostname();
    
    // Create a simple two-line display with hostname and "Hello World!"
    DisplayConfig {
        orientation: Orientation::Landscape,  // Default to landscape orientation
        width: 128,                          // Standard width
        height: 32,                          // Standard height
        elements: vec![
            // Hostname on the first line
            ElementConfig {
                id: "hostname".to_string(),
                position: PositionConfig {
                    x: PositionValue::Text("center".to_string()),
                    y: PositionValue::Number(8),  // Position for first line
                },
                components: vec![
                    ComponentConfig {
                        value: ValueConfig {
                            text: hostname,
                            font: "FONT_6X12".to_string(),
                        },
                        prefix: None,
                        suffix: None,
                    },
                ],
            },
            // Hello World on the second line
            ElementConfig {
                id: "hello_world".to_string(),
                position: PositionConfig {
                    x: PositionValue::Text("center".to_string()),
                    y: PositionValue::Number(22),  // Position for second line
                },
                components: vec![
                    ComponentConfig {
                        value: ValueConfig {
                            text: "Hello World!".to_string(),
                            font: "PCSENIOR8_STYLE".to_string(),
                        },
                        prefix: None,
                        suffix: None,
                    },
                ],
            },
        ],
    }
}

fn get_system_hostname() -> String {
    // Try to get the system hostname
    match Command::new("hostname").output() {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        },
        _ => "RustBerry".to_string(),  // Default if hostname command fails
    }
}