use super::values::{DisplayValues, ValueName};

/// Text from the config with `{name}` placeholders for values, parsed once
/// when the config is loaded. `{{` and `}}` stand for literal braces.
#[derive(Clone, Debug, PartialEq)]
pub struct Template {
    pieces: Vec<Piece>,
}

#[derive(Clone, Debug, PartialEq)]
enum Piece {
    Literal(String),
    Value(ValueName),
}

impl Template {
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut pieces = Vec::new();
        let mut literal = String::new();
        let mut chars = text.chars().peekable();

        while let Some(c) = chars.next() {
            match c {
                '{' if chars.next_if_eq(&'{').is_some() => literal.push('{'),
                '}' if chars.next_if_eq(&'}').is_some() => literal.push('}'),
                '{' => {
                    let name = value_name(&mut chars)?;
                    if !literal.is_empty() {
                        pieces.push(Piece::Literal(std::mem::take(&mut literal)));
                    }
                    pieces.push(Piece::Value(name));
                },
                '}' => return Err("lone \"}\" (write \"}}\" for a literal brace)".to_string()),
                _ => literal.push(c),
            }
        }
        if !literal.is_empty() {
            pieces.push(Piece::Literal(literal));
        }
        Ok(Template { pieces })
    }

    /// The text with each placeholder replaced by its value.
    pub fn render(&self, values: &DisplayValues) -> String {
        self.pieces.iter()
            .map(|piece| match piece {
                Piece::Literal(text) => text.clone(),
                Piece::Value(name) => values.get(*name),
            })
            .collect()
    }

    /// The characters drawn as written, without the values.
    pub fn literal_chars(&self) -> impl Iterator<Item = char> + '_ {
        self.pieces.iter()
            .filter_map(|piece| match piece {
                Piece::Literal(text) => Some(text.chars()),
                Piece::Value(_) => None,
            })
            .flatten()
    }
}

/// Reads a value name up to the closing `}`, the opening `{` already consumed.
fn value_name(chars: &mut impl Iterator<Item = char>) -> Result<ValueName, String> {
    let mut name = String::new();
    for c in chars.by_ref() {
        if c == '}' {
            if name.is_empty() {
                return Err("empty \"{}\"".to_string());
            }
            return ValueName::parse(&name).ok_or_else(|| format!("unknown value \"{{{}}}\"", name));
        }
        name.push(c);
    }
    Err(format!("unclosed \"{{{}\"", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn render(text: &str) -> String {
        Template::parse(text).unwrap().render(&DisplayValues::sample())
    }

    fn error(text: &str) -> String {
        Template::parse(text).unwrap_err()
    }

    #[test]
    fn parses_literals_and_values() {
        let template = Template::parse("{interface}: {ip}").unwrap();
        assert_eq!(template.pieces, vec![
            Piece::Value(ValueName::Interface), Piece::Literal(": ".to_string()), Piece::Value(ValueName::Ip)]);
        assert_eq!(template.render(&DisplayValues::sample()), "eth0.20: 10.0.20.133");
        assert_eq!(Template::parse("").unwrap().pieces, vec![]);
    }

    #[test]
    fn renders_templates() {
        assert_eq!(render("CPU"), "CPU");
        assert_eq!(render(".{ip_octet1}"), ".0");
        assert_eq!(render("{cpu_temp}°C"), "48.2°C");
        assert_eq!(render("{{cpu_temp}} {{}}"), "{cpu_temp} {}");
        assert_eq!(render("}}{{"), "}{");
    }

    #[test]
    fn lists_literal_characters() {
        let template = Template::parse("a{cpu_usage}b{{").unwrap();
        assert_eq!(template.literal_chars().collect::<String>(), "ab{");
    }

    #[test]
    fn rejects_bad_templates() {
        assert_eq!(error("{cpu_tmep}"), "unknown value \"{cpu_tmep}\"");
        assert_eq!(error("{ip_octets(0)}"), "unknown value \"{ip_octets(0)}\"");
        assert_eq!(error("x{}"), "empty \"{}\"");
        assert_eq!(error("{cpu_usage"), "unclosed \"{cpu_usage\"");
        assert_eq!(error("50}%"), "lone \"}\" (write \"}}\" for a literal brace)");
        assert!(error("{ cpu_usage }").contains("unknown value"));
    }
}
