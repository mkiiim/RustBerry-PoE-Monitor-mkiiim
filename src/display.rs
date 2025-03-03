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
use log::{debug, error};

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
        // let i2c = I2cdev::new("/dev/i2c-1")?;
        debug!("Initializing display with config path: {}", config_path);
        let i2c = I2cdev::new("/dev/i2c-1").map_err(|e| {
            error!("Failed to initialize I2C device: {}", e);
            e
        })?;

        let display = initialize_display(i2c)?;
        info!("Display initialized successfully");

        // Load config at initialization
        let mut file = File::open(config_path)?;
        let mut json_content = String::new();
        file.read_to_string(&mut json_content)?;
        let config: DisplayConfig = from_str(&json_content)?;

        info!("Loading config file from: {}", config_path);
        let mut file = match File::open(config_path) {
            Ok(f) => f,
            Err(e) => {
                error!("Failed to open config file {}: {}", config_path, e);
                return Err(Box::new(DisplayError::IoError(e)));
            }
        };

        let mut json_content = String::new();
        if let Err(e) = file.read_to_string(&mut json_content) {
            error!("Failed to read config file: {}", e);
            return Err(Box::new(DisplayError::IoError(e)));
        }

        info!("Parsing JSON config");
        let config: DisplayConfig = match from_str(&json_content) {
            Ok(c) => c,
            Err(e) => {
                error!("Failed to parse config JSON: {}", e);
                return Err(Box::new(DisplayError::JsonError(e)));
            }
        };
        info!("Config loaded successfully");


        Ok(PoeDisplay { display, config })
    }

    // Replace the entire update_display method with this improved version
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
    ) -> Result<(), DisplayError> {
        let disp = &mut self.display;
    
        // Always clear the entire display at the beginning
        disp.clear(BinaryColor::Off)?;
        
        // Use landscape orientation
        let orientation = &self.config.orientations.landscape;
        
        // Iterate over elements
        for element in &orientation.elements {
            let x_position = match &element.position.x {
                PositionValue::Text(val) => match val.as_str() {
                    "center" => (orientation.width - element.id.len() as i32 * 8) / 2,
                    "left" => 0,
                    "right" => orientation.width - element.id.len() as i32 * 8,
                    _ => 0,
                },
                PositionValue::Number(val) => *val,
            };   
    
            let y_position = match &element.position.y {
                PositionValue::Text(val) => match val.as_str() {
                    "incrementing" => 0, // Could be improved, but not using this now
                    _ => 0,
                },
                PositionValue::Number(val) => *val,
            };
    
            // Track the current horizontal position
            let mut current_x = x_position;
            
            // Draw components
            for component in &element.components {
                let value = match component.value.text.as_str() {
                    "interface_phys" => interface_phys,
                    "interface_numvlan" => interface_numvlan,
                    "ip_info.0" => &ip_info.0,
                    "ip_octets(0)" => &ip_octets[0].to_string(),
                    "ip_octets(1)" => &ip_octets[1].to_string(),
                    "ip_octets(2)" => &ip_octets[2].to_string(),
                    "ip_octets(3)" => &ip_octets[3].to_string(),
                    "cpu_usage" => &cpu_usage,
                    "cpu_temp" => &cpu_temp_str,
                    "ram_usage" => &ram_usage,
                    "disk_usage" => &disk_usage,
                    text => text,
                };
    
                let font = match component.value.font.as_str() {
                    "FONT_5X8" => FONT_5X8,
                    "FONT_6X12" => FONT_6X12,
                    "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                    "PROFONT12" => PROFONT12,
                    "PROFONT9" => PROFONT9,
                    _ => FONT_5X8,
                };
                
                // Get character width for main value font
                let char_width = get_char_width_from_text_style(&font);
                
                // Handle prefix if present (draw before the value)
                if let Some(prefix) = &component.prefix {
                    let prefix_font = match prefix.font.as_str() {
                        "FONT_5X8" => FONT_5X8,
                        "FONT_6X12" => FONT_6X12,
                        "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                        "PROFONT12" => PROFONT12,
                        "PROFONT9" => PROFONT9,
                        _ => FONT_5X8,
                    };
                    
                    // Get character width for prefix font
                    let prefix_char_width = get_char_width_from_text_style(&prefix_font);
                    
                    Text::new(&prefix.text, Point::new(current_x, y_position), prefix_font).draw(disp)?;
                    current_x += prefix.text.len() as i32 * prefix_char_width; // Advance with actual width
                }
                
                // Draw the main value
                Text::new(value, Point::new(current_x, y_position), font).draw(disp)?;
                current_x += value.len() as i32 * char_width; // Advance with actual width
                
                // Handle suffix if present (draw after the value)
                if let Some(suffix) = &component.suffix {
                    let suffix_font = match suffix.font.as_str() {
                        "FONT_5X8" => FONT_5X8,
                        "FONT_6X12" => FONT_6X12,
                        "PCSENIOR8_STYLE" => PCSENIOR8_STYLE,
                        "PROFONT12" => PROFONT12,
                        "PROFONT9" => PROFONT9,
                        _ => FONT_5X8,
                    };
                    
                    // Get character width for suffix font
                    let suffix_char_width = get_char_width_from_text_style(&suffix_font);
                    
                    Text::new(&suffix.text, Point::new(current_x, y_position), suffix_font).draw(disp)?;
                    current_x += suffix.text.len() as i32 * suffix_char_width; // Advance with actual width
                }
            }
        }
        
        // Ensure the buffer is fully flushed to the display
        disp.flush()?;
        
        Ok(())
    }

}

fn initialize_display(i2c: I2cdev) -> Result<Display, Box<dyn std::error::Error>> {
    let interface = I2CDisplayInterface::new(i2c);
    let mut disp = Ssd1306::new(interface, DisplaySize128x32, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

//    disp.init().map_err(|e| format!("Display initialization error: {:?}", e))?;
    <Ssd1306<_, _, _> as SsdDisplayConfig>::init(&mut disp)
        .map_err(|e| format!("Display initialization error: {:?}", e))?;
    Ok(disp)
}

fn get_char_width_from_text_style<'a>(font_style: &MonoTextStyle<'a, BinaryColor>) -> i32 {
    // Get the character width from the font's metadata
    // This includes both the character size and any additional spacing
    font_style.font.character_size.width as i32 + font_style.font.character_spacing as i32
}
