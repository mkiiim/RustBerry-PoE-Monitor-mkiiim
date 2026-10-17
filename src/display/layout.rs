use embedded_graphics::prelude::Point;

use super::config::{Align, Item, Line, Page, RowLine, Span};
use super::fonts::{text_width, Font};
use super::values::DisplayValues;

/// Text to draw with its left baseline at `position`.
pub struct TextRun {
    pub text: String,
    pub font: Font,
    pub position: Point,
    pub width: i32,
}

/// The runs of one config line, with where the line is in the config file.
pub struct LineRuns<'a> {
    pub path: &'a str,
    pub runs: Vec<TextRun>,
}

/// Works out what to draw where on `page`, for a display `display_width` wide.
pub fn layout(page: &Page, display_width: i32, values: &DisplayValues) -> Vec<TextRun> {
    layout_lines(page, display_width, values).into_iter().flat_map(|line| line.runs).collect()
}

/// Like [`layout`], keeping the runs of each line together.
///
/// A pen walks down the page from y = 0: each line moves it down by its
/// leading and is drawn with its baseline there, and each space moves it by
/// its size. Each line is aligned as a whole; a line that would start left
/// of the screen starts at the left edge instead, so its beginning stays readable.
pub fn layout_lines<'a>(page: &'a Page, display_width: i32, values: &DisplayValues) -> Vec<LineRuns<'a>> {
    let mut pen = 0;
    let mut lines = Vec::new();
    for line in &page.lines {
        match line {
            Line::Space(n) => pen += n,
            Line::Row(row) => {
                pen += row.leading;
                lines.push(LineRuns { path: &row.path, runs: place_line(row, pen, display_width, values) });
            },
        }
    }
    lines
}

/// The first run that doesn't fit within `display_width`, if any.
pub fn off_screen(runs: &[TextRun], display_width: i32) -> Option<&TextRun> {
    runs.iter().find(|run| run.position.x < 0 || run.position.x + run.width > display_width)
}

fn place_line(line: &RowLine, baseline: i32, display_width: i32, values: &DisplayValues) -> Vec<TextRun> {
    let x = if has_fill(&line.items) {
        0
    } else {
        let width = line.items.iter().map(|item| natural_width(item, values)).sum();
        align_at(line.align, 0, display_width, width).max(0)
    };
    let mut placer = Placer { values, baseline, runs: Vec::new() };
    placer.row(&line.items, x, Some(display_width));
    placer.runs
}

/// Collects the runs of one line as its items are placed left to right.
struct Placer<'a> {
    values: &'a DisplayValues,
    baseline: i32,
    runs: Vec<TextRun>,
}

impl Placer<'_> {
    /// Places `items` from `x` and returns where they end. Fills share the
    /// width the other items leave of `width`, if the row has one.
    fn row(&mut self, items: &[Item], x: i32, width: Option<i32>) -> i32 {
        let mut fills = fill_widths(items, width, self.values).into_iter();
        items.iter().fold(x, |x, item| match item {
            Item::Fill => x + fills.next().unwrap_or(0),
            _ => self.item(item, x),
        })
    }

    /// Places `item` at `x` and returns where it ends.
    fn item(&mut self, item: &Item, x: i32) -> i32 {
        match item {
            Item::Span(span) => self.span(span, x),
            Item::Row(items) => self.row(items, x, None),
            Item::Boxed { width, align, item } => {
                match item.as_ref() {
                    Item::Row(items) if has_fill(items) => {
                        self.row(items, x, Some(*width));
                    },
                    inner => {
                        let start = align_at(*align, x, *width, natural_width(inner, self.values));
                        self.item(inner, start);
                    },
                }
                x + width
            },
            Item::Space(n) => x + n,
            Item::Fill => x,
        }
    }

    fn span(&mut self, span: &Span, x: i32) -> i32 {
        let text = span.text.render(self.values);
        let font = span.font.style();
        let width = text_width(&text, &font);
        self.runs.push(TextRun { text, font, position: Point::new(x, self.baseline + span.dy), width });
        x + width
    }
}

/// Width of `item` without stretching any fills.
fn natural_width(item: &Item, values: &DisplayValues) -> i32 {
    match item {
        Item::Span(span) => text_width(&span.text.render(values), &span.font.style()),
        Item::Row(items) => items.iter().map(|item| natural_width(item, values)).sum(),
        Item::Boxed { width, .. } => *width,
        Item::Space(n) => *n,
        Item::Fill => 0,
    }
}

fn has_fill(items: &[Item]) -> bool {
    items.iter().any(|item| matches!(item, Item::Fill))
}

/// Widths of the fills in `items`, in order: the spare part of `width`
/// shared equally, the last fill taking what doesn't divide evenly.
/// Without a width, or without room to spare, fills are 0 wide.
fn fill_widths(items: &[Item], width: Option<i32>, values: &DisplayValues) -> Vec<i32> {
    let count = items.iter().filter(|item| matches!(item, Item::Fill)).count() as i32;
    let Some(width) = width.filter(|_| count > 0) else {
        return Vec::new();
    };
    let used: i32 = items.iter().map(|item| natural_width(item, values)).sum();
    let spare = (width - used).max(0);
    let share = spare / count;
    (1..=count).map(|i| if i == count { spare - share * (count - 1) } else { share }).collect()
}

/// Start of text `width` wide, placed in the span from `left` that is `span`
/// wide: at `left`, centred, or ending at `left + span`.
fn align_at(align: Align, left: i32, span: i32, width: i32) -> i32 {
    match align {
        Align::Left => left,
        Align::Center => left + (span - width) / 2,
        Align::Right => left + span - width,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display::config::DisplayConfig;

    const PORTRAIT: &str = include_str!("../../etc_rustberry-poe-monitor/portrait.json");
    const LANDSCAPE: &str = include_str!("../../etc_rustberry-poe-monitor/landscape.json");

    fn config(json: &str) -> DisplayConfig {
        DisplayConfig::parse(json).unwrap()
    }

    /// Text, x and y of each run on the first page of `config`.
    fn runs_of(config: &DisplayConfig, values: &DisplayValues) -> Vec<(String, i32, i32)> {
        layout(&config.pages[0], config.orientation.size().0, values).into_iter()
            .map(|r| (r.text, r.position.x, r.position.y))
            .collect()
    }

    /// Runs of a landscape config with `lines` as its lines.
    fn runs(lines: &str) -> Vec<(String, i32, i32)> {
        let json = format!(r#"{{"orientation": "landscape", "lines": {}}}"#, lines);
        runs_of(&config(&json), &DisplayValues::sample())
    }

    fn run(text: &str, x: i32, y: i32) -> (String, i32, i32) {
        (text.to_string(), x, y)
    }

    #[test]
    fn walks_the_pen_down_the_page() {
        // FONT_6X12 is 12px tall, FONT_5X8 8px; leading defaults to the tallest
        assert_eq!(runs(r#"["a", {"space": 3}, {"text": "b", "font": "FONT_5X8"}, {"space": -5},
                           {"leading": 4, "lines": ["c", {"text": "d", "leading": 20}]}, "e"]"#),
                   vec![run("a", 0, 12), run("b", 0, 23), run("c", 0, 22), run("d", 0, 42), run("e", 0, 54)]);
    }

    #[test]
    fn dy_moves_text_but_not_the_pen() {
        assert_eq!(runs(r#"[{"text": "a", "dy": 5}, {"row": ["b", {"text": "c", "dy": -2}], "dy": 1}, "d"]"#),
                   vec![run("a", 0, 17), run("b", 0, 25), run("c", 6, 23), run("d", 0, 36)]);
    }

    #[test]
    fn aligns_lines() {
        // "abcd" is 24px wide on the 128px display
        assert_eq!(runs(r#"["abcd", {"text": "abcd", "align": "center"}, {"text": "abcd", "align": "right"},
                           {"row": ["ab", {"space": 4}, "cd"], "align": "right"}]"#),
                   vec![run("abcd", 0, 12), run("abcd", 52, 24), run("abcd", 104, 36),
                        run("ab", 100, 48), run("cd", 116, 48)]);
    }

    #[test]
    fn keeps_start_of_wide_lines_on_screen() {
        let wide = "x".repeat(25); // 150px
        let lines = format!(r#"[{{"text": "{0}", "align": "center"}}, {{"row": ["{0}"], "align": "right"}},
                                {{"row": [{{"space": -10}}, "a"]}}]"#, wide);
        let runs = runs(&lines);
        assert_eq!((runs[0].1, runs[1].1), (0, 0));
        // Negative space inside a row is kept
        assert_eq!(runs[2], run("a", -10, 36));
    }

    #[test]
    fn shares_spare_width_among_fills() {
        // Two fills share 128 - 3 * 6 = 110 as 55 + 55; align has no effect.
        // Three share 128 - 2 * 6 = 116 as 38 + 38 + 40.
        assert_eq!(runs(r#"[{"row": ["a", {"fill": true}, "b", {"fill": true}, "c"], "align": "right"},
                           {"row": [{"fill": true}, "a", {"fill": true}, "b", {"fill": true}]},
                           {"row": [{"fill": true}, "abcd"]}]"#),
                   vec![run("a", 0, 12), run("b", 61, 12), run("c", 122, 12),
                        run("a", 38, 24), run("b", 82, 24),
                        run("abcd", 104, 36)]);
    }

    #[test]
    fn fills_never_shrink() {
        let wide = "x".repeat(25);
        let lines = format!(r#"[{{"row": ["{}", {{"fill": true}}, "a"]}}]"#, wide);
        assert_eq!(runs(&lines)[1], run("a", 150, 12));
    }

    #[test]
    fn fills_need_a_definite_width() {
        // An unboxed nested row has no width to fill; a boxed one does
        assert_eq!(runs(r#"[{"row": [{"row": ["a", {"fill": true}, "b"]}, "c"], "align": "right"},
                           {"row": [{"row": ["a", {"fill": true}, "b"], "width": 30}, "c"]}]"#),
                   vec![run("a", 110, 12), run("b", 116, 12), run("c", 122, 12),
                        run("a", 0, 24), run("b", 24, 24), run("c", 30, 24)]);
    }

    #[test]
    fn aligns_text_in_boxes() {
        assert_eq!(runs(r#"[{"row": [{"text": "ab", "width": 20}, {"text": "ab", "width": 20, "align": "center"},
                                     {"text": "ab", "width": 20, "align": "right"}, "c"]}]"#),
                   vec![run("ab", 0, 12), run("ab", 24, 12), run("ab", 48, 12), run("c", 60, 12)]);
        // Nested rows are boxed the same way, and text wider than its box overflows
        assert_eq!(runs(r#"[{"row": [{"space": 50}, {"row": ["ab", "c"], "width": 10, "align": "right"},
                                     {"text": "abc", "width": 10, "align": "center"}, "d"]}]"#),
                   vec![run("ab", 42, 12), run("c", 54, 12), run("abc", 56, 12), run("d", 70, 12)]);
    }

    #[test]
    fn lays_out_portrait_config() {
        let runs = runs_of(&config(PORTRAIT), &DisplayValues::sample());
        assert_eq!(runs.len(), 18);
        assert_eq!(runs[0], run("eth", 0, 8));
        assert_eq!(runs[1], run("0.20", 0, 16));
        assert_eq!(runs[2], run("10", 14, 26));
        assert_eq!(runs[3], run(".0", 8, 34));
        assert_eq!(runs[5], run(".133", 8, 50));
        assert_eq!(runs[6], run("CPU", 17, 63));            // right edge, 3 * 5px
        assert_eq!(runs[7], run("12.5", 0, 72));            // right-aligned in 24px, 4 * 6px
        assert_eq!(runs[8], run("%", 26, 72));
        assert_eq!(runs[9], run("48.2", 0, 81));
        assert_eq!(runs[10], run("o", 24, 77));
        assert_eq!(runs[11], run("C", 27, 81));
        assert_eq!(runs[12], run("RAM", 17, 93));
        assert_eq!(runs[13], run("7.0", 6, 102));
        assert_eq!(runs[15], run("DISK", 12, 114));
        assert_eq!(runs[16], run("21.4", 0, 123));
        assert_eq!(runs[17], run("%", 26, 123));
    }

    #[test]
    fn full_cpu_fits_in_portrait() {
        let full = DisplayValues { cpu_usage: "100".to_string(), ..DisplayValues::sample() };
        assert_eq!(runs_of(&config(PORTRAIT), &full)[7], run("100", 6, 72));
    }

    #[test]
    fn lays_out_landscape_config() {
        let runs = runs_of(&config(LANDSCAPE), &DisplayValues::sample());
        // "eth0.20: " is 9 * 5px and "10.0.20.133" 11 * 6px: 111px, centred at 8
        assert_eq!(runs[0..2], [run("eth0.20: ", 8, 7), run("10.0.20.133", 53, 7)]);
        assert_eq!(runs[2..5], [run("12.5", 6, 19), run("%", 40, 20), run("CPU", 46, 20)]);
        assert_eq!(runs[5..8], [run("48.2", 70, 19), run("°", 101, 22), run("C", 108, 19)]);
        assert_eq!(runs[8..11], [run("7.0", 14, 30), run("%", 40, 31), run("RAM", 46, 31)]);
        assert_eq!(runs[11..14], [run("21.4", 70, 30), run("%", 102, 31), run("DISK", 108, 31)]);
    }

    #[test]
    fn finds_text_off_screen() {
        let wide = DisplayValues {
            interface: "eth0.99".to_string(), ip: "192.168.100.200".to_string(), ..DisplayValues::sample()
        };
        let config = config(LANDSCAPE);
        let runs = layout(&config.pages[0], 128, &wide);
        // "eth0.99: " is 9 * 5px and "192.168.100.200" 15 * 6px, 135px in all
        assert_eq!((runs[0].text.as_str(), runs[0].position.x), ("eth0.99: ", 0));
        let last = off_screen(&runs, 128).unwrap();
        assert_eq!((last.text.as_str(), last.position.x), ("192.168.100.200", 45));

        assert!(off_screen(&layout(&config.pages[0], 128, &DisplayValues::sample()), 128).is_none());
        let portrait = self::config(PORTRAIT);
        assert!(off_screen(&layout(&portrait.pages[0], 32, &DisplayValues::sample()), 32).is_none());
    }
}
