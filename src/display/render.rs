use std::convert::Infallible;
use std::io::Write;
use embedded_graphics::{pixelcolor::BinaryColor, prelude::*};

use embedded_graphics::text::Text;

use super::config::{DisplayConfig, Orientation, Page};
use super::draw_runs;
use super::layout::{layout, layout_lines, TextRun};
use super::values::DisplayValues;

/// An in-memory screen the size of the display as it is read: 128x32 in
/// landscape, 32x128 in portrait. Pixels outside it are dropped, as on the OLED.
pub struct Canvas {
    size: Size,
    pixels: Vec<bool>,
}

impl Canvas {
    pub fn new(orientation: &Orientation) -> Self {
        let (width, height) = orientation.size();
        Self::with_size(Size::new(width as u32, height as u32))
    }

    fn with_size(size: Size) -> Self {
        Canvas { size, pixels: vec![false; (size.width * size.height) as usize] }
    }

    /// Positions of the lit pixels.
    fn lit_pixels(&self) -> impl Iterator<Item = (i32, i32)> + '_ {
        let width = self.size.width as usize;
        self.pixels.iter().enumerate()
            .filter(|(_, on)| **on)
            .map(move |(i, _)| ((i % width) as i32, (i / width) as i32))
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

/// Draws `page` with `values` onto a new canvas.
pub fn render(orientation: &Orientation, page: &Page, values: &DisplayValues) -> Canvas {
    let mut canvas = Canvas::new(orientation);
    let (width, _) = orientation.size();
    let Ok(()) = draw_runs(&mut canvas, &layout(page, width, values));
    canvas
}

/// Problems `config` would show with the widest values: text whose drawn
/// pixels run past the sides, top or bottom of the display. For `--check-config`.
pub fn check_layout(config: &DisplayConfig) -> Vec<String> {
    let (width, height) = config.orientation.size();
    let values = DisplayValues::worst_case();
    let mut warnings = Vec::new();

    for (number, page) in (1..).zip(&config.pages) {
        for line in layout_lines(page, width, &values) {
            let spills: Vec<_> = line.runs.iter().map(|run| (run, spill(run, width, height))).collect();
            if let Some((run, _)) = spills.iter().find(|(_, (sides, _))| *sides) {
                warnings.push(format!("page {}, {}: \"{}\" runs past the edge of the {}px wide display",
                                      number, line.path, run.text, width));
            }
            if let Some((run, _)) = spills.iter().find(|(_, (_, ends))| *ends) {
                warnings.push(format!("page {}, {}: \"{}\" runs past the top or bottom of the {}px high display",
                                      number, line.path, run.text, height));
            }
        }
    }
    warnings
}

/// Whether the pixels `run` draws fall past the sides, and past the top or
/// bottom, of a display `width` x `height`. Blank parts of the font's
/// character cells don't count.
fn spill(run: &TextRun, width: i32, height: i32) -> (bool, bool) {
    let font = run.font.font;
    let top = run.position.y - font.baseline as i32;
    // A canvas covering both the display and the run's character cells
    let (left, up) = (run.position.x.min(0), top.min(0));
    let right = (run.position.x + run.width).max(width);
    let down = (top + font.character_size.height as i32).max(height);
    let mut canvas = Canvas::with_size(Size::new((right - left) as u32, (down - up) as u32));
    let Ok(_) = Text::new(&run.text, run.position - Point::new(left, up), run.font).draw(&mut canvas);

    canvas.lit_pixels()
        .map(|(x, y)| (x + left, y + up))
        .fold((false, false), |(sides, ends), (x, y)| {
            (sides || x < 0 || x >= width, ends || y < 0 || y >= height)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::config::DisplayConfig;

    fn values() -> DisplayValues {
        DisplayValues::sample()
    }

    fn lit(canvas: &Canvas) -> usize {
        canvas.pixels.iter().filter(|on| **on).count()
    }

    #[test]
    fn renders_shipped_configs() {
        let portrait = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/portrait.json")).unwrap();
        let canvas = render(&portrait.orientation, &portrait.pages[0], &values());
        assert_eq!(canvas.size(), Size::new(32, 128));
        assert!(lit(&canvas) > 200, "{}", lit(&canvas));

        let landscape = DisplayConfig::parse(include_str!("../../etc_rustberry-poe-monitor/landscape.json")).unwrap();
        assert_eq!(render(&landscape.orientation, &landscape.pages[0], &values()).size(), Size::new(128, 32));
    }

    fn config(json: &str) -> DisplayConfig {
        DisplayConfig::parse(json).unwrap()
    }

    #[test]
    fn warns_about_text_off_screen() {
        let config = config(r#"{"orientation": "portrait", "pages": [{"lines": ["ok"]},
            {"lines": [{"text": "X", "font": "FONT_5X8", "leading": 3}, {"text": "{ip}", "align": "right"},
                       {"space": 100}, {"row": ["{ip_octet0}", {"text": "y", "dy": 5}]}]}]}"#);
        assert_eq!(check_layout(&config), vec![
            "page 2, pages[1].lines[0]: \"X\" runs past the top or bottom of the 128px high display".to_string(),
            "page 2, pages[1].lines[1]: \"255.255.255.255\" runs past the edge of the 32px wide display".to_string(),
            "page 2, pages[1].lines[3]: \"y\" runs past the top or bottom of the 128px high display".to_string(),
        ]);
    }

    #[test]
    fn ignores_blank_parts_of_character_cells() {
        // FONT_6X12 at baseline 7 has its cell from row -2, but nothing is drawn there
        let config = config(r#"{"orientation": "landscape", "lines": [{"text": "Ag", "leading": 7}]}"#);
        assert!(check_layout(&config).is_empty());
    }

    #[test]
    fn warns_about_shipped_layouts_only_with_widest_values() {
        let portrait = config(include_str!("../../etc_rustberry-poe-monitor/portrait.json"));
        assert_eq!(check_layout(&portrait), vec![
            "page 1, lines[0].lines[1]: \"0.999\" runs past the edge of the 32px wide display".to_string()]);
        let landscape = config(include_str!("../../etc_rustberry-poe-monitor/landscape.json"));
        assert_eq!(check_layout(&landscape), vec![
            "page 1, lines[0]: \"255.255.255.255\" runs past the edge of the 128px wide display".to_string()]);
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
