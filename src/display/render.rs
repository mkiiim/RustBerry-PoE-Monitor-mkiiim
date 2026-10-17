use std::convert::Infallible;
use std::io::Write;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use super::config::{DisplayConfig, Orientation};
use super::draw_runs;
use super::layout::{layout, DisplayValues};

/// An in-memory screen the size of the display as it is read: 128x32 in
/// landscape, 32x128 in portrait. Pixels outside it are dropped, as on the OLED.
pub struct Canvas {
    size: Size,
    pixels: Vec<bool>,
}

impl Canvas {
    pub fn new(orientation: &Orientation) -> Self {
        let size = match orientation {
            Orientation::Landscape => Size::new(128, 32),
            Orientation::Portrait => Size::new(32, 128),
        };
        Canvas { size, pixels: vec![false; (size.width * size.height) as usize] }
    }

    pub fn is_on(&self, x: u32, y: u32) -> bool {
        self.pixels[(y * self.size.width + x) as usize]
    }

    /// Writes the canvas as a greyscale PNG, each pixel drawn `scale` x `scale`.
    pub fn write_png<W: Write>(&self, out: W, scale: u32) -> Result<(), png::EncodingError> {
        let (width, height) = (self.size.width * scale, self.size.height * scale);
        let mut encoder = png::Encoder::new(out, width, height);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Eight);

        let data: Vec<u8> = (0..height)
            .flat_map(|y| (0..width).map(move |x| (x / scale, y / scale)))
            .map(|(x, y)| if self.is_on(x, y) { 255 } else { 0 })
            .collect();
        encoder.write_header()?.write_image_data(&data)
    }
}

impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        self.size
    }
}

impl DrawTarget for Canvas {
    type Color = BinaryColor;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if let (Ok(x), Ok(y)) = (u32::try_from(point.x), u32::try_from(point.y)) {
                if x < self.size.width && y < self.size.height {
                    self.pixels[(y * self.size.width + x) as usize] = color.is_on();
                }
            }
        }
        Ok(())
    }
}

/// Draws `config` with `values` onto a new canvas.
pub fn render(config: &DisplayConfig, values: &DisplayValues) -> Canvas {
    let mut canvas = Canvas::new(&config.orientation);
    let Ok(()) = draw_runs(&mut canvas, &layout(config, values));
    canvas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn values() -> DisplayValues {
        DisplayValues::sample()
    }

    fn lit(canvas: &Canvas) -> usize {
        canvas.pixels.iter().filter(|on| **on).count()
    }

    #[test]
    fn renders_shipped_configs() {
        let portrait = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/portrait.json")).unwrap();
        let canvas = render(&portrait, &values());
        assert_eq!(canvas.size(), Size::new(32, 128));
        assert!(lit(&canvas) > 200, "{}", lit(&canvas));

        let landscape = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/landscape.json")).unwrap();
        assert_eq!(render(&landscape, &values()).size(), Size::new(128, 32));
    }

    #[test]
    fn drops_pixels_off_the_canvas() {
        let mut canvas = Canvas::new(&Orientation::Landscape);
        let pixels = [Pixel(Point::new(-1, 0), BinaryColor::On), Pixel(Point::new(128, 0), BinaryColor::On),
                      Pixel(Point::new(0, 32), BinaryColor::On), Pixel(Point::new(127, 31), BinaryColor::On)];
        canvas.draw_iter(pixels).unwrap();
        assert_eq!(lit(&canvas), 1);
        assert!(canvas.is_on(127, 31));
    }

    #[test]
    fn writes_scaled_png() {
        let mut canvas = Canvas::new(&Orientation::Portrait);
        canvas.draw_iter([Pixel(Point::new(1, 0), BinaryColor::On)]).unwrap();
        let mut bytes = Vec::new();
        canvas.write_png(&mut bytes, 4).unwrap();

        let mut reader = png::Decoder::new(bytes.as_slice()).read_info().unwrap();
        let mut data = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut data).unwrap();
        assert_eq!((info.width, info.height), (128, 512));
        // Pixel (1, 0) covers columns 4..8 of the first four rows
        assert_eq!(&data[..9], &[0, 0, 0, 0, 255, 255, 255, 255, 0]);
        assert_eq!(data[3 * 128 + 4], 255);
        assert_eq!(data[4 * 128 + 4], 0);
    }
}
