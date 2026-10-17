use super::config::DisplayConfig;
use super::DisplayError;

/// Layout shown when the config file can't be loaded: the hostname and
/// "CONFIG ERROR", so the problem is visible when the screen is woken.
const CONFIG_ERROR_LAYOUT: &str = r#"{
  "orientation": "landscape",
  "align": "center",
  "lines": [
    { "text": "{hostname}", "font": "FONT_6X12", "leading": 8 },
    { "text": "CONFIG ERROR", "font": "PCSENIOR8_STYLE", "leading": 14 }
  ]
}"#;

pub fn config_error_display_config() -> Result<DisplayConfig, DisplayError> {
    DisplayConfig::parse(CONFIG_ERROR_LAYOUT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::layout::layout;
    use crate::display::values::DisplayValues;

    #[test]
    fn shows_hostname_and_error() {
        let config = config_error_display_config().unwrap();
        let values = DisplayValues { hostname: "rpi-pihole2".to_string(), ..DisplayValues::sample() };
        let runs = layout(&config.pages[0], 128, &values);
        let runs: Vec<_> = runs.iter().map(|r| (r.text.as_str(), r.position.x, r.position.y)).collect();
        // 11 * 6px and 12 * 8px wide, centred on 128px
        assert_eq!(runs, vec![("rpi-pihole2", 31, 8), ("CONFIG ERROR", 16, 22)]);
    }
}
