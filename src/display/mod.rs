mod config;
mod default_config;
mod fonts;
mod layout;

pub use layout::DisplayValues;

use linux_embedded_hal::I2cdev;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306, mode::BufferedGraphicsMode};
use ssd1306::mode::DisplayConfig as SsdDisplayConfig;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, text::Text};
use log::{debug, error, info, warn};

use config::DisplayConfig;
use default_config::get_default_display_config;
use layout::layout;

type Display = Ssd1306<I2CInterface<I2cdev>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

#[derive(Debug)]
pub enum DisplayError {
    IoError(std::io::Error),
    JsonError(serde_json::Error),
    DisplayError(display_interface::DisplayError),
}

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayError::IoError(e) => write!(f, "IO error: {}", e),
            DisplayError::JsonError(e) => write!(f, "JSON error: {}", e),
            DisplayError::DisplayError(e) => write!(f, "Display error: {:?}", e),
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

impl From<display_interface::DisplayError> for DisplayError {
    fn from(error: display_interface::DisplayError) -> Self {
        DisplayError::DisplayError(error)
    }
}

pub struct PoeDisplay {
    display: Display,
    config: DisplayConfig,
}

impl PoeDisplay {
    /// Opens the OLED and loads the layout from `config_path`, falling back
    /// to a built-in layout if the file cannot be read.
    pub fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Initializing display with config path: {}", config_path);

        let config = match DisplayConfig::load(config_path) {
            Ok(config) => {
                info!("Configuration loaded successfully from: {}", config_path);
                config
            },
            Err(e) => {
                warn!("Failed to load config from {}: {}", config_path, e);
                warn!("Using default display configuration instead");
                get_default_display_config()
            }
        };

        debug!("Config details: orientation={:?}, width={}, height={}, elements={}",
               config.orientation, config.width, config.height, config.elements.len());

        let i2c = I2cdev::new("/dev/i2c-1").map_err(|e| {
            error!("Failed to initialize I2C device: {}", e);
            e
        })?;

        let display = initialize_display(i2c, &config)?;
        info!("Display initialized successfully");

        Ok(PoeDisplay { display, config })
    }

    /// Blanks the screen and switches the panel off. The fan and everything
    /// else keep running; only drawing stops.
    pub fn power_off(&mut self) -> Result<(), DisplayError> {
        self.display.clear(BinaryColor::Off)?;
        self.display.flush()?;
        self.display.set_display_on(false)?;
        Ok(())
    }

    /// Redraws the whole screen with `values`.
    pub fn update_display(&mut self, values: &DisplayValues) -> Result<(), DisplayError> {
        self.display.clear(BinaryColor::Off)?;
        for run in layout(&self.config, values) {
            Text::new(&run.text, run.position, run.font).draw(&mut self.display)?;
        }
        self.display.flush()?;
        Ok(())
    }
}

fn initialize_display(i2c: I2cdev, config: &DisplayConfig) -> Result<Display, Box<dyn std::error::Error>> {
    let interface = I2CDisplayInterface::new(i2c);

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
