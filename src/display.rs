use log::info;
use crate::display_types::{DisplayConfig, Display, FONT_5X8, FONT_6X12, PCSENIOR8_STYLE, PROFONT12, PROFONT9, PositionValue};
use linux_embedded_hal::I2cdev;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use ssd1306::mode::DisplayConfig as SsdDisplayConfig;
use display_interface::DisplayError as InterfaceDisplayError;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    mono_font::MonoTextStyle,
    text::Text
};
use std::fs::File;
use std::io::Read;
use serde_json::from_str;
use log::{debug, error, warn};

// Use the default display config module
use crate::default_config::get_default_display_config;

#[derive(Debug)]
pub enum DisplayError {
    InvalidOrientation,
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    DisplayError(display_interface::DisplayError),
    ConfigError(String),
}

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayError::InvalidOrientation => write!(f, "Invalid orientation"),
            DisplayError::IoError(e) => write!(f, "IO error: {}", e),
            DisplayError::JsonError(e) => write!(f, "JSON error: {}", e),
            DisplayError::DisplayError(e) => write!(f, "Display error: {:?}", e),
            DisplayError::ConfigError(e) => write!(f, "Config error: {}", e),
        }
    }
}

impl std::error::Error for DisplayError {}

impl From<std::io::Error> for DisplayError {
    fn from(error: std::io::Error) -> Self {
        DisplayError::IoError(error)
    }
}

impl From<serde_json::Error> for DisplayError {
    fn from(error: serde_json::Error) -> Self {
        DisplayError::JsonError(error)
    }
}

impl From<InterfaceDisplayError> for DisplayError {
    fn from(_error: InterfaceDisplayError) -> Self {
        DisplayError::DisplayError(_error)
    }
}

pub struct PoeDisplay {
    display: Display,
    config: DisplayConfig,
}

impl PoeDisplay {
    pub fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Initializing display with config path: {}", config_path);
        
        // Attempt to load config from file
        let config = match Self::load_config_from_file(config_path) {
            Ok(config) => {
                info!("Configuration loaded successfully from: {}", config_path);
                config
            },
            Err(e) => {
                // Log the error but continue with default config
                warn!("Failed to load config from {}: {}", config_path, e);
                warn!("Using default display configuration instead");
                get_default_display_config()
            }
        };
        
        debug!("Config details: orientation={:?}, width={}, height={}, elements={}", 
               config.orientation, config.width, config.height, config.elements.len());
        
        // Initialize I2C
        let i2c = I2cdev::new("/dev/i2c-1").map_err(|e| {
            error!("Failed to initialize I2C device: {}", e);
            e
        })?;

        // Initialize display with orientation from config
        let display = initialize_display(i2c, &config)?;
        info!("Display initialized successfully");

        Ok(PoeDisplay { display, config })
    }
    
    // Helper method to load config from file
    fn load_config_from_file(config_path: &str) -> Result<DisplayConfig, Box<dyn std::error::Error>> {
        info!("Loading config file from: {}", config_path);
        let mut file = File::open(config_path)?;

        let mut json_content = String::new();
        file.read_to_string(&mut json_content)?;

        info!("Parsing JSON config");
        let config: DisplayConfig = from_str::<DisplayConfig>(&json_content)?;
        
        info!("Configuration loaded with orientation: {:?}", config.orientation);
        Ok(config)
    }

    pub fn update_display(
        &mut self,
        ip_info: &(String, String, [u8; 4]),
        ip_address: &str,
        interface: &str,
        interface_phys: &str,
        interface_numvlan: &str,
        ip_octets: &[u8; 4],
        cpu_usage: &String,
        cpu_temp_str: &String,
        ram_usage: &String,
        disk_usage: &str,
        display_orientation: &str,
    ) -> Result<(), DisplayError> {
        let disp = &mut self.display;
    
        // Always clear the entire display at the beginning
        disp.clear(BinaryColor::Off)?;
        
        // Iterate over elements
        for element in &self.config.elements {
            // First, prepare all components by resolving values and calculating their widths
            struct PreparedComponent {
                value_text: String,
                value_font: MonoTextStyle<'static, BinaryColor>,
                value_width: i32,
                prefix_text: Option<String>,
                prefix_font: Option<MonoTextStyle<'static, BinaryColor>>,
                prefix_width: i32,
                suffix_text: Option<String>,
                suffix_font: Option<MonoTextStyle<'static, BinaryColor>>,
                suffix_width: i32,
                total_width: i32,
            }
            
            let mut prepared_components = Vec::new();
            let mut total_element_width = 0;
            
            for component in &element.components {
                // Resolve the actual value text
                let value_text = match component.value.text.as_str() {
                    "interface_phys" => interface_phys.to_string(),
                    "interface_numvlan" => interface_numvlan.to_string(),
                    "ip_info.0" => ip_info.0.clone(),
                    "ip_octets(0)" => ip_octets[0].to_string(),
                    "ip_octets(1)" => ip_octets[1].to_string(),
                    "ip_octets(2)" => ip_octets[2].to_string(),
                    "ip_octets(3)" => ip_octets[3].to_string(),
                    "cpu_usage" => cpu_usage.clone(),
                    "cpu_temp" => cpu_temp_str.clone(),
                    "ram_usage" => ram_usage.clone(),
                    "disk_usage" => disk_usage.to_string(),
                    text => text.to_string(),
                };
                
                // Get the font for the value - keep this exactly as it was
                let value_font = match component.value.font.as_str() {
                    "FONT_5X8" => FONT_5X8,
                    "FONT_6X12" => FONT_6X12,
                    "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                    "PROFONT12" => PROFONT12,
                    "PROFONT9" => PROFONT9,
                    _ => FONT_5X8,
                };
                
                // Calculate value width
                let char_width = get_char_width_from_text_style(&value_font);
                let value_width = value_text.len() as i32 * char_width;
                
                // Process prefix if present
                let (prefix_text, prefix_font, prefix_width) = if let Some(prefix) = &component.prefix {
                    let prefix_font = match prefix.font.as_str() {
                        "FONT_5X8" => FONT_5X8,
                        "FONT_6X12" => FONT_6X12,
                        "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                        "PROFONT12" => PROFONT12,
                        "PROFONT9" => PROFONT9,
                        _ => FONT_5X8,
                    };
                    
                    let prefix_char_width = get_char_width_from_text_style(&prefix_font);
                    let prefix_width = prefix.text.len() as i32 * prefix_char_width;
                    
                    (Some(prefix.text.clone()), Some(prefix_font), prefix_width)
                } else {
                    (None, None, 0)
                };
                
                // Process suffix if present
                let (suffix_text, suffix_font, suffix_width) = if let Some(suffix) = &component.suffix {
                    let suffix_font = match suffix.font.as_str() {
                        "FONT_5X8" => FONT_5X8,
                        "FONT_6X12" => FONT_6X12,
                        "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                        "PROFONT12" => PROFONT12,
                        "PROFONT9" => PROFONT9,
                        _ => FONT_5X8,
                    };
                    
                    let suffix_char_width = get_char_width_from_text_style(&suffix_font);
                    let suffix_width = suffix.text.len() as i32 * suffix_char_width;
                    
                    (Some(suffix.text.clone()), Some(suffix_font), suffix_width)
                } else {
                    (None, None, 0)
                };
                
                // Calculate total width for this component
                let component_total_width = prefix_width + value_width + suffix_width;
                total_element_width += component_total_width;
                
                // Store the prepared component
                prepared_components.push(PreparedComponent {
                    value_text,
                    value_font,
                    value_width,
                    prefix_text,
                    prefix_font,
                    prefix_width,
                    suffix_text,
                    suffix_font,
                    suffix_width,
                    total_width: component_total_width,
                });
            }
            
            // Calculate the starting x position based on alignment
            let x_position = match &element.position.x {
                PositionValue::Text(val) => match val.as_str() {
                    "center" => (self.config.width - total_element_width) / 2,
                    "left" => 0,
                    "right" => self.config.width - total_element_width,
                    _ => 0,
                },
                PositionValue::Number(val) => *val,
                PositionValue::Relative { align, anchor } => match align.as_str() {
                    "center" => anchor - (total_element_width / 2),
                    "left" => *anchor,
                    "right" => anchor - total_element_width,
                    _ => *anchor,
                }
            };
            
            let y_position = match &element.position.y {
                PositionValue::Text(val) => match val.as_str() {
                    "incrementing" => 0, // Could be improved
                    _ => 0,
                },
                PositionValue::Number(val) => *val,
                PositionValue::Relative { align: _, anchor } => *anchor,
            };
            
            // Draw all components with the correct positioning
            let mut current_x = x_position;
            
            for component in prepared_components {
                // Draw prefix if present
                if let (Some(prefix_text), Some(prefix_font)) = (component.prefix_text, component.prefix_font) {
                    Text::new(&prefix_text, Point::new(current_x, y_position), prefix_font).draw(disp)?;
                    current_x += component.prefix_width;
                }
                
                // Draw value
                Text::new(&component.value_text, Point::new(current_x, y_position), component.value_font).draw(disp)?;
                current_x += component.value_width;
                
                // Draw suffix if present
                if let (Some(suffix_text), Some(suffix_font)) = (component.suffix_text, component.suffix_font) {
                    Text::new(&suffix_text, Point::new(current_x, y_position), suffix_font).draw(disp)?;
                    current_x += component.suffix_width;
                }
            }
        }
        
        // Ensure the buffer is fully flushed to the display
        disp.flush()?;
        
        Ok(())
    }
}

fn initialize_display(i2c: I2cdev, config: &DisplayConfig) -> Result<Display, Box<dyn std::error::Error>> {
    let interface = I2CDisplayInterface::new(i2c);

    // Use rotation from config
    let rotation = config.orientation.to_display_rotation();
    debug!("Initializing display with rotation: {:?} based on orientation: {:?}", 
           rotation, config.orientation);

    let mut disp = Ssd1306::new(interface, DisplaySize128x32, rotation)
        .into_buffered_graphics_mode();

    <Ssd1306<_, _, _> as SsdDisplayConfig>::init(&mut disp)
        .map_err(|e| format!("Display initialization error: {:?}", e))?;
    debug!("Display successfully initialized");
    Ok(disp)
}

fn get_char_width_from_text_style<'a>(font_style: &MonoTextStyle<'a, BinaryColor>) -> i32 {
    // Get the character width from the font's metadata
    // This includes both the character size and any additional spacing
    font_style.font.character_size.width as i32 + font_style.font.character_spacing as i32
}