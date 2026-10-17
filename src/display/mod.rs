mod config;
mod default_config;
mod fonts;
mod layout;
mod render;
mod template;
mod values;

use std::fs::File;
use std::io::{BufWriter, Write};
use std::time::Instant;

pub use values::DisplayValues;

use linux_embedded_hal::I2cdev;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306, mode::BufferedGraphicsMode};
use ssd1306::mode::DisplayConfig as SsdDisplayConfig;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*, text::Text};
use log::{debug, error, info, warn};

use config::DisplayConfig;
use default_config::config_error_display_config;
use layout::{layout, off_screen, TextRun};
use render::check_layout;

type Display = Ssd1306<I2CInterface<I2cdev>, DisplaySize128x32, BufferedGraphicsMode<DisplaySize128x32>>;

#[derive(Debug)]
pub enum DisplayError {
    Io(std::io::Error),
    Json(serde_json::Error),
    Interface(display_interface::DisplayError),
    Config(String),
}

impl std::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DisplayError::Io(e) => write!(f, "IO error: {}", e),
            DisplayError::Json(e) => write!(f, "JSON error: {}", e),
            DisplayError::Interface(e) => write!(f, "Display error: {:?}", e),
            DisplayError::Config(e) => write!(f, "Config error: {}", e),
        }
    }
}

impl std::error::Error for DisplayError {}

impl From<std::io::Error> for DisplayError {
    fn from(error: std::io::Error) -> Self {
        DisplayError::Io(error)
    }
}

impl From<serde_json::Error> for DisplayError {
    fn from(error: serde_json::Error) -> Self {
        DisplayError::Json(error)
    }
}

impl From<display_interface::DisplayError> for DisplayError {
    fn from(error: display_interface::DisplayError) -> Self {
        DisplayError::Interface(error)
    }
}

/// Loads the config at `path` without touching the display, for `--check-config`.
/// Returns warnings about text that may not fit on the screen.
pub fn check_config(path: &str) -> Result<Vec<String>, DisplayError> {
    DisplayConfig::load(path).map(|config| check_layout(&config))
}

/// How many PNG pixels `--render` draws per display pixel.
const RENDER_SCALE: u32 = 4;

/// Draws page `page` (from 1) of the layout in `config_path` with `values`
/// to a PNG at `out_path`, without touching the display, for `--render`.
pub fn render_png(config_path: &str, page: usize, values: &DisplayValues, out_path: &str)
                  -> Result<(), Box<dyn std::error::Error>> {
    let config = DisplayConfig::load(config_path)?;
    let count = config.pages.len();
    let page = page.checked_sub(1).and_then(|i| config.pages.get(i))
        .ok_or_else(|| format!("no page {}, the config has {} page{}", page, count, if count == 1 { "" } else { "s" }))?;
    let mut out = BufWriter::new(File::create(out_path)?);
    render::render(&config.orientation, page, values).write_png(&mut out, RENDER_SCALE)?;
    out.flush()?;
    Ok(())
}

/// Draws `runs` onto the OLED, or onto a canvas for `--render`.
fn draw_runs<D: DrawTarget<Color = BinaryColor>>(target: &mut D, runs: &[TextRun]) -> Result<(), D::Error> {
    for run in runs {
        Text::new(&run.text, run.position, run.font).draw(target)?;
    }
    Ok(())
}

pub struct PoeDisplay {
    display: Display,
    config: DisplayConfig,
    /// When the display was set up; pages turn over counting from here
    started: Instant,
    overflow_warned: bool,
}

impl PoeDisplay {
    /// Opens the OLED and loads the layout from `config_path`. If the file
    /// can't be loaded, the screen shows the hostname and "CONFIG ERROR" instead.
    pub fn new(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        debug!("Initializing display with config path: {}", config_path);

        let config = match DisplayConfig::load(config_path) {
            Ok(config) => {
                info!("Configuration loaded successfully from: {}", config_path);
                config
            },
            Err(e) => {
                error!("Failed to load config from {}: {}", config_path, e);
                config_error_display_config()?
            }
        };

        debug!("Config details: orientation={:?}, pages={}, page_seconds={}",
               config.orientation, config.pages.len(), config.page_seconds);

        let i2c = I2cdev::new("/dev/i2c-1").map_err(|e| {
            error!("Failed to initialize I2C device: {}", e);
            e
        })?;

        let display = initialize_display(i2c, &config)?;
        info!("Display initialized successfully");

        Ok(PoeDisplay { display, config, started: Instant::now(), overflow_warned: false })
    }

    /// Blanks the screen and switches the panel off. The fan and everything
    /// else keep running; only drawing stops.
    pub fn power_off(&mut self) -> Result<(), DisplayError> {
        self.display.clear(BinaryColor::Off)?;
        self.display.flush()?;
        self.display.set_display_on(false)?;
        Ok(())
    }

    /// Switches the panel back on, showing whatever was last drawn.
    pub fn power_on(&mut self) -> Result<(), DisplayError> {
        self.display.set_display_on(true)?;
        Ok(())
    }

    /// Redraws the whole screen with `values`, on the page due now.
    pub fn update_display(&mut self, values: &DisplayValues) -> Result<(), DisplayError> {
        self.display.clear(BinaryColor::Off)?;
        let (width, _) = self.config.orientation.size();
        let runs = layout(self.config.page_at(self.started.elapsed()), width, values);
        if !self.overflow_warned {
            if let Some(run) = off_screen(&runs, width) {
                warn!("\"{}\" doesn't fit on the {}px wide display", run.text, width);
                self.overflow_warned = true;
            }
        }
        draw_runs(&mut self.display, &runs)?;
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
