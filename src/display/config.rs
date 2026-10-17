use std::fs;
use std::time::Duration;
use ssd1306::prelude::DisplayRotation;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{Map, Value};
use log::info;

use super::DisplayError;
use super::fonts::{can_draw, FontName};
use super::template::Template;

#[derive(Deserialize, Debug, PartialEq)]
pub enum Orientation {
    #[serde(rename = "landscape")]
    Landscape,
    #[serde(rename = "portrait")]
    Portrait,
}

impl Orientation {
    pub fn to_display_rotation(&self) -> DisplayRotation {
        match self {
            Orientation::Portrait => DisplayRotation::Rotate90,
            Orientation::Landscape => DisplayRotation::Rotate0,
        }
    }

    /// Width and height of the display as it is read.
    pub fn size(&self) -> (i32, i32) {
        match self {
            Orientation::Portrait => (32, 128),
            Orientation::Landscape => (128, 32),
        }
    }
}

#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    Left,
    Center,
    Right,
}

/// Display layout loaded from a JSON file such as `/etc/rustberry-poe-monitor/portrait.json`.
/// The file is checked completely when it is loaded, and inherited fonts,
/// alignment and leading are resolved, so laying it out can't fail.
#[derive(Debug)]
pub struct DisplayConfig {
    pub orientation: Orientation,
    /// How long each page is shown when there are several
    pub page_seconds: u64,
    /// One or more pages, never empty
    pub pages: Vec<Page>,
}

/// One screenful of lines, drawn from the top.
#[derive(Debug)]
pub struct Page {
    pub lines: Vec<Line>,
}

/// A step down the page. Groups in the config are flattened into their lines.
#[derive(Debug)]
pub enum Line {
    /// Moves the pen down by this many pixels, or up if negative
    Space(i32),
    /// Text drawn at one baseline. A text line is a row with one span.
    Row(RowLine),
}

#[derive(Debug)]
pub struct RowLine {
    /// Where the line is in the config file, e.g. `lines[3].lines[1]`
    pub path: String,
    pub align: Align,
    /// How far the pen moves down to this line's baseline
    pub leading: i32,
    pub items: Vec<Item>,
}

/// Something placed left to right in a row.
#[derive(Debug)]
pub enum Item {
    Span(Span),
    Row(Vec<Item>),
    /// `item` aligned inside a box `width` pixels wide
    Boxed { width: i32, align: Align, item: Box<Item> },
    /// A horizontal gap, may be negative
    Space(i32),
    /// A flexible gap sharing the row's spare width
    Fill,
}

/// Text in one font, `dy` pixels below the line's baseline.
#[derive(Debug)]
pub struct Span {
    pub text: Template,
    pub font: FontName,
    pub dy: i32,
}

const DEFAULT_PAGE_SECONDS: u64 = 5;

const OLD_FORMAT: &str = "old layout format, see the README (Display Configuration)";

impl DisplayConfig {
    pub fn load(path: &str) -> Result<Self, DisplayError> {
        info!("Loading config file from: {}", path);
        let config = Self::parse(&fs::read_to_string(path)?)?;
        info!("Configuration loaded with orientation: {:?}", config.orientation);
        Ok(config)
    }

    pub fn parse(json: &str) -> Result<Self, DisplayError> {
        let value: Value = serde_json::from_str(json)?;
        Self::from_value(&value).map_err(DisplayError::Config)
    }

    /// The page to show `elapsed` after the display started.
    pub fn page_at(&self, elapsed: Duration) -> &Page {
        let index = (elapsed.as_secs() / self.page_seconds) % self.pages.len() as u64;
        &self.pages[index as usize]
    }

    fn from_value(value: &Value) -> Result<Self, String> {
        let root = object(value, "config")?;
        if root.contains_key("elements") {
            return Err(OLD_FORMAT.to_string());
        }
        check_fields(root, "", &["orientation", "font", "align", "leading", "page_seconds", "lines", "pages"])?;

        let orientation = root.get("orientation")
            .ok_or_else(|| "missing field \"orientation\"".to_string())
            .and_then(|value| parse_enum(value, "orientation"))?;
        let page_seconds = match root.get("page_seconds") {
            Some(value) => positive(value, "page_seconds")? as u64,
            None => DEFAULT_PAGE_SECONDS,
        };
        let inherited = Inherited { font: FontName::Font6x12, align: Align::Left, leading: None }
            .with(root, "")?;
        let pages = match (root.get("lines"), root.get("pages")) {
            (Some(lines), None) => vec![Page { lines: parse_lines(lines, "lines", inherited)? }],
            (None, Some(pages)) => parse_pages(pages, inherited)?,
            _ => return Err("expected exactly one of \"lines\" or \"pages\"".to_string()),
        };
        Ok(DisplayConfig { orientation, page_seconds, pages })
    }
}

/// Font, alignment and leading passed down from the top level, pages and groups.
#[derive(Clone, Copy)]
struct Inherited {
    font: FontName,
    align: Align,
    leading: Option<i32>,
}

impl Inherited {
    /// These properties overridden by any set in `map`.
    fn with(self, map: &Map<String, Value>, path: &str) -> Result<Self, String> {
        Ok(Inherited {
            font: optional(map, path, "font", parse_enum)?.unwrap_or(self.font),
            align: optional(map, path, "align", parse_enum)?.unwrap_or(self.align),
            leading: optional(map, path, "leading", positive)?.or(self.leading),
        })
    }
}

fn parse_pages(value: &Value, inherited: Inherited) -> Result<Vec<Page>, String> {
    non_empty_array(value, "pages")?.iter().enumerate()
        .map(|(i, page)| {
            let path = format!("pages[{}]", i);
            let map = object(page, &path)?;
            check_fields(map, &path, &["lines", "font", "align", "leading"])?;
            let lines = map.get("lines").ok_or_else(|| format!("{}: missing field \"lines\"", path))?;
            Ok(Page { lines: parse_lines(lines, &field(&path, "lines"), inherited.with(map, &path)?)? })
        })
        .collect()
}

fn parse_lines(value: &Value, path: &str, inherited: Inherited) -> Result<Vec<Line>, String> {
    let mut lines = Vec::new();
    for (i, line) in non_empty_array(value, path)?.iter().enumerate() {
        parse_line(line, &format!("{}[{}]", path, i), inherited, &mut lines)?;
    }
    Ok(lines)
}

/// Parses one entry of a `lines` array onto `lines`; a group adds all of its lines.
fn parse_line(value: &Value, path: &str, inherited: Inherited, lines: &mut Vec<Line>) -> Result<(), String> {
    if value.is_string() {
        let items = vec![Item::Span(parse_span(value, path, inherited.font, 0)?)];
        lines.push(Line::Row(row_line(path, inherited, items)));
        return Ok(());
    }
    let map = object(value, path)?;
    match shape(map, path, &["text", "row", "lines", "space"])? {
        "text" => {
            check_fields(map, path, &["text", "font", "align", "leading", "dy"])?;
            let inherited = inherited.with(map, path)?;
            let dy = optional(map, path, "dy", integer)?.unwrap_or(0);
            let span = parse_span(&map["text"], &field(path, "text"), inherited.font, dy)?;
            lines.push(Line::Row(row_line(path, inherited, vec![Item::Span(span)])));
        },
        "row" => {
            check_fields(map, path, &["row", "font", "align", "leading", "dy"])?;
            let inherited = inherited.with(map, path)?;
            let dy = optional(map, path, "dy", integer)?.unwrap_or(0);
            let items = parse_row(&map["row"], &field(path, "row"), inherited.font, dy)?;
            lines.push(Line::Row(row_line(path, inherited, items)));
        },
        "lines" => {
            check_fields(map, path, &["lines", "font", "align", "leading"])?;
            lines.extend(parse_lines(&map["lines"], &field(path, "lines"), inherited.with(map, path)?)?);
        },
        _ => {
            check_fields(map, path, &["space"])?;
            lines.push(Line::Space(integer(&map["space"], &field(path, "space"))?));
        },
    }
    Ok(())
}

/// A text or row line. Without a leading set, the line is as tall as the
/// tallest font used in it, or its own font if it has no text.
fn row_line(path: &str, inherited: Inherited, items: Vec<Item>) -> RowLine {
    let leading = inherited.leading.unwrap_or_else(|| {
        let mut fonts = Vec::new();
        collect_fonts(&items, &mut fonts);
        fonts.iter()
            .map(|font| font.style().font.character_size.height as i32)
            .max()
            .unwrap_or(inherited.font.style().font.character_size.height as i32)
    });
    RowLine { path: path.to_string(), align: inherited.align, leading, items }
}

fn collect_fonts(items: &[Item], fonts: &mut Vec<FontName>) {
    for item in items {
        match item {
            Item::Span(span) => fonts.push(span.font),
            Item::Row(items) => collect_fonts(items, fonts),
            Item::Boxed { item, .. } => collect_fonts(std::slice::from_ref(item.as_ref()), fonts),
            Item::Space(_) | Item::Fill => {},
        }
    }
}

/// The items of a `row` array, in `font` unless they set their own.
fn parse_row(value: &Value, path: &str, font: FontName, dy: i32) -> Result<Vec<Item>, String> {
    let array = value.as_array().ok_or_else(|| format!("{}: expected an array", path))?;
    array.iter().enumerate()
        .map(|(i, item)| parse_item(item, &format!("{}[{}]", path, i), font, dy))
        .collect()
}

fn parse_item(value: &Value, path: &str, font: FontName, dy: i32) -> Result<Item, String> {
    if value.is_string() {
        return Ok(Item::Span(parse_span(value, path, font, dy)?));
    }
    let map = object(value, path)?;
    if map.contains_key("lines") {
        return Err(format!("{}: \"lines\" can't be used inside a row", path));
    }
    match shape(map, path, &["text", "row", "space", "fill"])? {
        "text" => {
            check_fields(map, path, &["text", "font", "dy", "width", "align"])?;
            let font = optional(map, path, "font", parse_enum)?.unwrap_or(font);
            let dy = dy + optional(map, path, "dy", integer)?.unwrap_or(0);
            let span = parse_span(&map["text"], &field(path, "text"), font, dy)?;
            boxed(map, path, Item::Span(span))
        },
        "row" => {
            check_fields(map, path, &["row", "font", "dy", "width", "align"])?;
            let font = optional(map, path, "font", parse_enum)?.unwrap_or(font);
            let dy = dy + optional(map, path, "dy", integer)?.unwrap_or(0);
            let items = parse_row(&map["row"], &field(path, "row"), font, dy)?;
            boxed(map, path, Item::Row(items))
        },
        "space" => {
            check_fields(map, path, &["space"])?;
            Ok(Item::Space(integer(&map["space"], &field(path, "space"))?))
        },
        _ => {
            check_fields(map, path, &["fill"])?;
            match map["fill"] {
                Value::Bool(true) => Ok(Item::Fill),
                _ => Err(format!("{}: \"fill\" must be true", path)),
            }
        },
    }
}

/// `item` in a box if `map` sets a `width`, aligned inside it by `align`.
fn boxed(map: &Map<String, Value>, path: &str, item: Item) -> Result<Item, String> {
    let align = optional(map, path, "align", parse_enum)?;
    match optional(map, path, "width", integer)? {
        Some(width) if width < 0 => Err(format!("{}: \"width\" can't be negative", field(path, "width"))),
        Some(width) => Ok(Item::Boxed { width, align: align.unwrap_or(Align::Left), item: Box::new(item) }),
        None if align.is_some() => Err(format!("{}: \"align\" needs a \"width\" inside a row", path)),
        None => Ok(item),
    }
}

/// A span of text, checking the font can draw each character written in it.
fn parse_span(value: &Value, path: &str, font: FontName, dy: i32) -> Result<Span, String> {
    let text = value.as_str().ok_or_else(|| format!("{}: expected a string", path))?;
    let text = Template::parse(text).map_err(|e| format!("{}: {}", path, e))?;
    let style = font.style();
    if let Some(c) = text.literal_chars().find(|c| !can_draw(&style, *c)) {
        return Err(format!("{}: {} can't draw \"{}\"", path, font.name(), c));
    }
    Ok(Span { text, font, dy })
}

/// The one key of `shapes` that `map` has, which says what kind of entry it is.
fn shape<'a>(map: &Map<String, Value>, path: &str, shapes: &[&'a str]) -> Result<&'a str, String> {
    let found: Vec<&str> = shapes.iter().copied().filter(|key| map.contains_key(*key)).collect();
    match found[..] {
        [one] => Ok(one),
        _ => Err(format!("{}: expected exactly one of {}, found {}", path, quoted(shapes),
                         if found.is_empty() { "none".to_string() } else { quoted(&found) })),
    }
}

fn quoted(keys: &[&str]) -> String {
    keys.iter().map(|key| format!("\"{}\"", key)).collect::<Vec<_>>().join(", ")
}

fn check_fields(map: &Map<String, Value>, path: &str, allowed: &[&str]) -> Result<(), String> {
    match map.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(format!("{}: unknown field", field(path, key))),
        None => Ok(()),
    }
}

/// `map[key]` parsed by `parse`, if present.
fn optional<T>(map: &Map<String, Value>, path: &str, key: &str,
               parse: fn(&Value, &str) -> Result<T, String>) -> Result<Option<T>, String> {
    map.get(key).map(|value| parse(value, &field(path, key))).transpose()
}

fn object<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, String> {
    value.as_object().ok_or_else(|| format!("{}: expected an object", path))
}

fn non_empty_array<'a>(value: &'a Value, path: &str) -> Result<&'a Vec<Value>, String> {
    match value.as_array() {
        Some(array) if array.is_empty() => Err(format!("{}: can't be empty", path)),
        Some(array) => Ok(array),
        None => Err(format!("{}: expected an array", path)),
    }
}

fn integer(value: &Value, path: &str) -> Result<i32, String> {
    value.as_i64()
        .and_then(|n| i32::try_from(n).ok())
        .ok_or_else(|| format!("{}: expected an integer", path))
}

fn positive(value: &Value, path: &str) -> Result<i32, String> {
    match integer(value, path)? {
        n if n < 1 => Err(format!("{}: must be at least 1", path)),
        n => Ok(n),
    }
}

/// A font, alignment or orientation name.
fn parse_enum<T: DeserializeOwned>(value: &Value, path: &str) -> Result<T, String> {
    T::deserialize(value).map_err(|e| format!("{}: {}", path, e))
}

/// The path of `key` inside the entry at `path`.
fn field(path: &str, key: &str) -> String {
    if path.is_empty() {
        key.to_string()
    } else {
        format!("{}.{}", path, key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> DisplayConfig {
        DisplayConfig::parse(json).unwrap()
    }

    fn error(json: &str) -> String {
        match DisplayConfig::parse(json) {
            Ok(_) => panic!("accepted: {}", json),
            Err(e) => e.to_string(),
        }
    }

    /// A landscape config with `lines` as its lines.
    fn with_lines(lines: &str) -> String {
        format!(r#"{{"orientation": "landscape", "lines": {}}}"#, lines)
    }

    fn line_error(lines: &str) -> String {
        error(&with_lines(lines))
    }

    fn rows(config: &DisplayConfig) -> Vec<&RowLine> {
        config.pages[0].lines.iter()
            .filter_map(|line| match line {
                Line::Row(row) => Some(row),
                Line::Space(_) => None,
            })
            .collect()
    }

    fn span(item: &Item) -> &Span {
        match item {
            Item::Span(span) => span,
            Item::Boxed { item, .. } => span(item),
            _ => panic!("not a span: {:?}", item),
        }
    }

    #[test]
    fn parses_shipped_configs() {
        let portrait = parse(include_str!("../../etc_rustberry-poe-monitor/portrait.json"));
        assert_eq!(portrait.orientation, Orientation::Portrait);
        assert_eq!(portrait.orientation.size(), (32, 128));
        assert_eq!(portrait.pages.len(), 1);

        let landscape = parse(include_str!("../../etc_rustberry-poe-monitor/landscape.json"));
        assert_eq!(landscape.orientation, Orientation::Landscape);
        assert_eq!(landscape.orientation.size(), (128, 32));
        assert_eq!(rows(&landscape).len(), 3);
    }

    #[test]
    fn applies_defaults() {
        let config = parse(&with_lines(r#"["{ip}"]"#));
        assert_eq!(config.page_seconds, 5);
        let line = rows(&config)[0];
        assert_eq!((line.path.as_str(), line.align, line.leading), ("lines[0]", Align::Left, 12));
        assert_eq!(span(&line.items[0]).font, FontName::Font6x12);
    }

    #[test]
    fn inherits_font_align_and_leading() {
        let config = parse(r#"{"orientation": "landscape", "font": "FONT_5X8", "align": "right", "leading": 9,
            "pages": [{"align": "center", "lines": [
                "a",
                {"font": "PROFONT9", "leading": 7, "lines": [
                    "b",
                    {"text": "c", "font": "FONT_6X12", "align": "left", "leading": 3}]},
                {"row": ["d", {"text": "e", "font": "PCSENIOR8_STYLE"}, {"row": ["f"]}], "font": "PROFONT12"}]}]}"#);
        let lines = rows(&config);
        let summary: Vec<_> = lines.iter()
            .map(|line| (line.path.as_str(), line.align, line.leading, span(&line.items[0]).font))
            .collect();
        assert_eq!(summary, vec![
            ("pages[0].lines[0]", Align::Center, 9, FontName::Font5x8),
            ("pages[0].lines[1].lines[0]", Align::Center, 7, FontName::ProFont9),
            ("pages[0].lines[1].lines[1]", Align::Left, 3, FontName::Font6x12),
            ("pages[0].lines[2]", Align::Center, 9, FontName::ProFont12),
        ]);
        assert_eq!(span(&lines[3].items[1]).font, FontName::PcSenior8);
        match &lines[3].items[2] {
            Item::Row(items) => assert_eq!(span(&items[0]).font, FontName::ProFont12),
            item => panic!("not a row: {:?}", item),
        }
    }

    #[test]
    fn leading_defaults_to_tallest_font_in_line() {
        let config = parse(&with_lines(r#"[{"row": ["a", {"text": "b", "font": "PROFONT12"}], "font": "FONT_5X8"},
            {"text": "c", "font": "FONT_5X8"}, {"row": [{"text": "d", "font": "FONT_5X8"}]},
            {"row": [], "font": "PCSENIOR8_STYLE"}]"#));
        let leadings: Vec<i32> = rows(&config).iter().map(|line| line.leading).collect();
        assert_eq!(leadings, vec![15, 8, 8, 10]);
    }

    #[test]
    fn adds_up_dy() {
        let config = parse(&with_lines(r#"[{"row": [{"row": [{"text": "a", "dy": 3}, "b"], "dy": -1}, "c"], "dy": 10},
            {"text": "d", "dy": 2}]"#));
        let lines = rows(&config);
        let Item::Row(nested) = &lines[0].items[0] else { panic!("not a row") };
        assert_eq!((span(&nested[0]).dy, span(&nested[1]).dy, span(&lines[0].items[1]).dy), (12, 9, 10));
        assert_eq!(span(&lines[1].items[0]).dy, 2);
    }

    #[test]
    fn parses_row_items() {
        let config = parse(&with_lines(r#"[{"row": [{"space": -2}, {"fill": true},
            {"text": "x", "width": 20, "align": "right"}, {"row": [], "width": 5}]}]"#));
        let items = &rows(&config)[0].items;
        assert!(matches!(items[0], Item::Space(-2)));
        assert!(matches!(items[1], Item::Fill));
        assert!(matches!(items[2], Item::Boxed { width: 20, align: Align::Right, .. }));
        assert!(matches!(items[3], Item::Boxed { width: 5, align: Align::Left, .. }));
    }

    #[test]
    fn parses_pages() {
        let config = parse(r#"{"orientation": "portrait", "page_seconds": 2,
            "pages": [{"lines": ["a"]}, {"lines": ["b", {"space": -3}]}]}"#);
        assert_eq!(config.page_seconds, 2);
        assert_eq!(config.pages.len(), 2);
        assert!(matches!(config.pages[1].lines[1], Line::Space(-3)));
    }

    #[test]
    fn selects_page_by_time() {
        let config = parse(r#"{"orientation": "portrait", "page_seconds": 2,
            "pages": [{"lines": ["a"]}, {"lines": ["b"]}, {"lines": ["c"]}]}"#);
        let page = |secs: u64| rows_of(config.page_at(Duration::from_secs(secs)));
        assert_eq!(page(0), "pages[0].lines[0]");
        assert_eq!(page(1), "pages[0].lines[0]");
        assert_eq!(page(2), "pages[1].lines[0]");
        assert_eq!(page(5), "pages[2].lines[0]");
        assert_eq!(page(6), "pages[0].lines[0]");

        let single = parse(&with_lines(r#"["a"]"#));
        assert!(std::ptr::eq(single.page_at(Duration::from_secs(1234)), &single.pages[0]));
    }

    fn rows_of(page: &Page) -> &str {
        match &page.lines[0] {
            Line::Row(row) => &row.path,
            Line::Space(_) => panic!("space"),
        }
    }

    #[test]
    fn rejects_old_format() {
        let e = error(&include_str!("../../etc_rustberry-poe-monitor/portrait.json")
            .replacen("\"lines\"", "\"elements\"", 1));
        assert_eq!(e, "Config error: old layout format, see the README (Display Configuration)");
    }

    #[test]
    fn rejects_bad_top_level() {
        assert_eq!(error(r#"{"lines": ["a"]}"#), "Config error: missing field \"orientation\"");
        assert!(error(r#"{"orientation": "sideways", "lines": ["a"]}"#).contains("orientation: unknown variant `sideways`"));
        assert_eq!(error(r#"{"orientation": "portrait", "width": 32, "lines": ["a"]}"#),
                   "Config error: width: unknown field");
        assert_eq!(error(r#"{"orientation": "portrait"}"#),
                   "Config error: expected exactly one of \"lines\" or \"pages\"");
        assert_eq!(error(r#"{"orientation": "portrait", "lines": ["a"], "pages": [{"lines": ["b"]}]}"#),
                   "Config error: expected exactly one of \"lines\" or \"pages\"");
        assert_eq!(error(r#"{"orientation": "portrait", "page_seconds": 0, "lines": ["a"]}"#),
                   "Config error: page_seconds: must be at least 1");
        assert_eq!(error("[]"), "Config error: config: expected an object");
        assert!(error("{").starts_with("JSON error"));
    }

    #[test]
    fn rejects_empty_arrays() {
        assert_eq!(line_error("[]"), "Config error: lines: can't be empty");
        assert_eq!(error(r#"{"orientation": "portrait", "pages": []}"#), "Config error: pages: can't be empty");
        assert_eq!(error(r#"{"orientation": "portrait", "pages": [{"lines": []}]}"#),
                   "Config error: pages[0].lines: can't be empty");
        assert_eq!(line_error(r#"["a", {"lines": []}]"#), "Config error: lines[1].lines: can't be empty");
        assert_eq!(error(r#"{"orientation": "portrait", "pages": [{"font": "FONT_5X8"}]}"#),
                   "Config error: pages[0]: missing field \"lines\"");
    }

    #[test]
    fn rejects_unknown_fields() {
        assert_eq!(line_error(r#"[{"text": "a", "fnt": "FONT_5X8"}]"#), "Config error: lines[0].fnt: unknown field");
        assert_eq!(line_error(r#"[{"lines": ["a"], "dy": 1}]"#), "Config error: lines[0].dy: unknown field");
        assert_eq!(line_error(r#"[{"space": 1, "font": "FONT_5X8"}]"#), "Config error: lines[0].font: unknown field");
        assert_eq!(line_error(r#"[{"row": [{"text": "a", "leading": 2}]}]"#),
                   "Config error: lines[0].row[0].leading: unknown field");
        assert_eq!(line_error(r#"[{"text": "a", "width": 20}]"#), "Config error: lines[0].width: unknown field");
        assert_eq!(line_error(r#"[{"row": [{"fill": true, "width": 3}]}]"#),
                   "Config error: lines[0].row[0].width: unknown field");
    }

    #[test]
    fn rejects_wrong_shapes() {
        assert_eq!(line_error("[3]"), "Config error: lines[0]: expected an object");
        assert_eq!(line_error(r#"[{"font": "FONT_5X8"}]"#),
                   "Config error: lines[0]: expected exactly one of \"text\", \"row\", \"lines\", \"space\", found none");
        assert_eq!(line_error(r#"[{"text": "a", "row": []}]"#),
                   "Config error: lines[0]: expected exactly one of \"text\", \"row\", \"lines\", \"space\", found \"text\", \"row\"");
        assert_eq!(line_error(r#"[{"row": [{"text": "a", "space": 1}]}]"#),
                   "Config error: lines[0].row[0]: expected exactly one of \"text\", \"row\", \"space\", \"fill\", found \"text\", \"space\"");
        assert_eq!(line_error(r#"[{"text": 5}]"#), "Config error: lines[0].text: expected a string");
        assert_eq!(line_error(r#"[{"row": "a"}]"#), "Config error: lines[0].row: expected an array");
        assert_eq!(line_error(r#"[{"space": "a"}]"#), "Config error: lines[0].space: expected an integer");
        assert_eq!(line_error(r#"[{"text": "a", "dy": 1.5}]"#), "Config error: lines[0].dy: expected an integer");
    }

    #[test]
    fn rejects_bad_fonts_and_alignment() {
        let e = line_error(r#"["a", {"text": "b", "font": "FONT_6x12"}]"#);
        assert!(e.starts_with("Config error: lines[1].font: unknown variant `FONT_6x12`"), "{}", e);
        let e = line_error(r#"[{"row": [{"text": "a", "width": 9, "align": "centre"}]}]"#);
        assert!(e.starts_with("Config error: lines[0].row[0].align: unknown variant `centre`"), "{}", e);
    }

    #[test]
    fn rejects_bad_row_items() {
        assert_eq!(line_error(r#"[{"row": [{"fill": false}]}]"#), "Config error: lines[0].row[0]: \"fill\" must be true");
        assert_eq!(line_error(r#"[{"row": [{"fill": 1}]}]"#), "Config error: lines[0].row[0]: \"fill\" must be true");
        assert_eq!(line_error(r#"[{"row": ["a", {"text": "b", "align": "right"}]}]"#),
                   "Config error: lines[0].row[1]: \"align\" needs a \"width\" inside a row");
        assert_eq!(line_error(r#"[{"row": [{"row": [{"lines": ["a"]}]}]}]"#),
                   "Config error: lines[0].row[0].row[0]: \"lines\" can't be used inside a row");
        assert_eq!(line_error(r#"[{"row": [{"text": "a", "width": -1}]}]"#),
                   "Config error: lines[0].row[0].width: \"width\" can't be negative");
    }

    #[test]
    fn rejects_small_leading() {
        assert_eq!(line_error(r#"[{"text": "a", "leading": 0}]"#), "Config error: lines[0].leading: must be at least 1");
        assert_eq!(error(r#"{"orientation": "portrait", "leading": -2, "lines": ["a"]}"#),
                   "Config error: leading: must be at least 1");
    }

    #[test]
    fn rejects_bad_templates() {
        assert_eq!(line_error(r#"["{cpu_tmep}"]"#), "Config error: lines[0]: unknown value \"{cpu_tmep}\"");
        assert_eq!(line_error(r#"[{"row": ["a", {"text": "{}"}]}]"#), "Config error: lines[0].row[1].text: empty \"{}\"");
        assert_eq!(line_error(r#"[{"text": "{ip"}]"#), "Config error: lines[0].text: unclosed \"{ip\"");
        assert!(line_error(r#"["a}"]"#).starts_with("Config error: lines[0]: lone \"}\""));
    }

    #[test]
    fn rejects_characters_the_font_lacks() {
        assert_eq!(line_error(r#"[{"text": "{cpu_temp}°C", "font": "FONT_5X8"}]"#),
                   "Config error: lines[0].text: FONT_5X8 can't draw \"°\"");
        assert_eq!(line_error(r#"[{"row": [{"text": "°", "font": "FONT_6X12"}]}]"#),
                   "Config error: lines[0].row[0].text: FONT_6X12 can't draw \"°\"");
        assert!(DisplayConfig::parse(&with_lines(r#"[{"row": [{"text": "°", "font": "PROFONT12"}]}]"#)).is_ok());
        assert!(DisplayConfig::parse(&with_lines(r#"[{"text": "?", "font": "FONT_5X8"}]"#)).is_ok());
    }
}
