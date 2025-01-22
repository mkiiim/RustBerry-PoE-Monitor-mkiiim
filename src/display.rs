use crate::display_types::{Display, FONT_5X8, FONT_6X12, PCSENIOR8_STYLE, PROFONT12, PROFONT9};
use linux_embedded_hal::I2cdev;
use ssd1306::{prelude::*, I2CDisplayInterface, Ssd1306};
use display_interface::DisplayError as InterfaceDisplayError;
use embedded_graphics::{
    pixelcolor::BinaryColor,
    prelude::*,
    text::Text
};

#[derive(Debug)]
pub enum DisplayError {
    InvalidOrientation,
    // Other variants...
}

impl From<InterfaceDisplayError> for DisplayError {
    fn from(error: InterfaceDisplayError) -> Self {
        // Map the InterfaceDisplayError variants to your DisplayError variants
        // For now, you can use a generic mapping
        DisplayError::InvalidOrientation // Adjust this mapping as needed
    }
}

pub struct PoeDisplay {
    display: Display
}

impl PoeDisplay {
    pub fn new(display_mode: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let i2c = I2cdev::new("/dev/i2c-1")?;
        let display = initialize_display(i2c, display_mode)?;
        Ok(PoeDisplay { display })
    }

    pub fn update_display(
        &mut self,
        _ip_address: &str,
        _interface: &str,
        interface_phys: &str,
        interface_numvlan: &str,
        _ip_octets: [u8; 4],
        cpu_usage: String,
        temp: String,
        ram_usage: String,
        disk_usage: &str,
        display_orientation: &str,
    ) -> Result<(), DisplayError> {
        let disp = &mut self.display;

        let y_offset = 7;
        let mut y_increment = 0;
        let display_width = 128;
        let char_width: i32 = 8;
        let interface_char_width: i32 = 5;
        let x_margin = Point::new(2, 0).x_axis();
        let y_margin = Point::new(0, 1).y_axis();

        disp.clear(BinaryColor::Off)?;

        // Apply different configurations based on display_orientation
        match display_orientation {
            "landscape" => {
                // top center: interface
                let interface_width = _interface.len() as i32 * interface_char_width;
                let interface_x_position = (display_width - interface_width) / 2;
                Text::new(_interface, Point::new(interface_x_position, y_offset), FONT_5X8).draw(disp)?;

                // middle left: cpu usage
                let cpu_width = cpu_usage.len() as i32 * char_width;
                let cpu_point = Point::new(34 - cpu_width, 12 + y_offset);
                let next = Text::new(&cpu_usage, cpu_point, PCSENIOR8_STYLE).draw(disp)?;
                let next = Text::new("%", next + y_margin, FONT_6X12).draw(disp)?;
                Text::new("CPU", next + x_margin, FONT_5X8).draw(disp)?;

                // bottom left: ram usage
                let ram_width = ram_usage.len() as i32 * char_width;
                let ram_point = Point::new(34 - ram_width, 23 + y_offset);
                let next = Text::new(&ram_usage, ram_point, PCSENIOR8_STYLE).draw(disp)?;
                let next = Text::new("%", next + y_margin, FONT_6X12).draw(disp)?;
                Text::new("RAM", next + x_margin, FONT_5X8).draw(disp)?;

                // middle right: temp
                let temp_width = temp.len() as i32 * char_width;
                let temp_point = Point::new(99 - temp_width, 12 + y_offset);
                let next = Text::new(&temp, temp_point, PCSENIOR8_STYLE).draw(disp)?;
                let next = Text::new("°", next + Point::new(0, 3), PROFONT12).draw(disp)?;
                Text::new("C", next - Point::new(0, 2), PCSENIOR8_STYLE).draw(disp)?;

                // bottom right: disk usage
                let disk_width = disk_usage.len() as i32 * char_width;
                let disk_point = Point::new(99 - disk_width, 23 + y_offset);
                let next = Text::new(disk_usage, disk_point, PCSENIOR8_STYLE).draw(disp)?;
                let next = Text::new("%", next + y_margin, FONT_6X12).draw(disp)?;
                Text::new("DISK", next + x_margin, FONT_5X8).draw(disp)?;
            }
            "portrait" => {
                let display_width = 32; // Assuming the width is 32 for portrait mode
                let display_height = 128; // Assuming the height is 128 for portrait mode
                y_increment += y_offset;
                
                // Interface Block
                let interface_width = interface_phys.len() as i32 * char_width;
                let interface_x_position = display_width - interface_width;
                Text::new(interface_phys, Point::new(interface_x_position, y_increment), PCSENIOR8_STYLE).draw(disp)?;
                y_increment += 9;
                
                if interface_numvlan.len() > 0 {
                    let interface_width = interface_numvlan.len() as i32 * char_width;
                    let interface_x_position = display_width - interface_width;
                    Text::new(interface_numvlan, Point::new(interface_x_position, y_increment), PCSENIOR8_STYLE).draw(disp)?;
                    y_increment += 10;
                } else {
                    y_increment += 1;
                }
                
                for i in 0.._ip_octets.len() {
                    let mut octet_str = _ip_octets[i].to_string();
                    // if i < _ip_octets.len() - 1 {
                        octet_str.push('.');
                        // }
                        let octet_str_ref: &str = &octet_str;
                        let interface_width = octet_str_ref.len() as i32 * interface_char_width;
                        let interface_x_position = display_width - interface_width ;
                        Text::new(octet_str_ref, Point::new(interface_x_position, y_increment ), FONT_5X8).draw(disp)?;
                        y_increment += 8;
                    }
                    y_increment += 3;
                    
                // CPU block
                let cpu_title = "CPU";
                let cpu_title_width = cpu_title.len() as i32 * char_width;
                let cpu_title_x_position = display_width - cpu_title_width;
                let cpu_usage_width = cpu_usage.len() as i32 * interface_char_width;
                let cpu_usage_x_position = (display_width * 3 / 4) - cpu_usage_width;
                let cpu_temp_width = temp.len() as i32 * interface_char_width;
                let cpu_temp_x_position = (display_width * 3 / 4) - cpu_temp_width;
                
                Text::new(cpu_title, Point::new(cpu_title_x_position, y_increment), PCSENIOR8_STYLE).draw(disp)?;
                y_increment += 9;
                let next = Text::new(&cpu_usage, Point::new (cpu_usage_x_position, y_increment), FONT_5X8).draw(disp)?;
                Text::new("%", next + x_margin, FONT_5X8).draw(disp)?;
                y_increment += 8;
                let next = Text::new(&temp, Point::new(cpu_temp_x_position, y_increment), FONT_5X8).draw(disp)?;
                Text::new("°", next + Point::new(0, 3), PROFONT9).draw(disp)?;
                y_increment += 11;
                
                // RAM block
                let ram_title = "RAM";
                let ram_title_width = ram_title.len() as i32 * char_width;
                let ram_title_x_position = display_width - ram_title_width;
                let ram_width = ram_usage.len() as i32 * interface_char_width;
                let ram_x_position = (display_width * 3 / 4) - ram_width;
                
                Text::new(&ram_title, Point::new(ram_title_x_position, y_increment), PCSENIOR8_STYLE).draw(disp)?;
                y_increment += 9;
                let next = Text::new(&ram_usage, Point::new(ram_x_position, y_increment), FONT_5X8).draw(disp)?;
                Text::new("%", next + y_margin, FONT_5X8).draw(disp)?;
                y_increment += 11;
                
                // Disk block
                let disk_title = "DISK";
                let disk_title_width = disk_title.len() as i32 * char_width;
                let disk_title_x_position = display_width - disk_title_width;
                let disk_width = disk_usage.len() as i32 * interface_char_width;
                let disk_x_position = (display_width * 3 / 4) - disk_width;
                
                Text::new(disk_title, Point::new(disk_title_x_position, y_increment), PCSENIOR8_STYLE).draw(disp)?;
                y_increment += 9;
                let next = Text::new(&disk_usage, Point::new(disk_x_position, y_increment), FONT_5X8).draw(disp)?;
                Text::new("%", next + y_margin, FONT_5X8).draw(disp)?;
                y_increment += 10;
                
            }
            _ => {
                return Err(DisplayError::InvalidOrientation);
            }
        }

        disp.flush()?;

        Ok(())
    }
}

fn initialize_display(i2c: I2cdev, display: &str) -> Result<Display, Box<dyn std::error::Error>> {
    let interface = I2CDisplayInterface::new(i2c);
    let rotation = match display {
        "landscape" => DisplayRotation::Rotate0,
        "portrait" => DisplayRotation::Rotate90,
        _ => DisplayRotation::Rotate0, // Default to landscape
    };
    let mut disp = Ssd1306::new(interface, DisplaySize128x32, rotation)
        .into_buffered_graphics_mode();

    disp.init().map_err(|e| format!("Display initialization error: {:?}", e))?;
    Ok(disp)
}

