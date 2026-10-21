//! Line syntax of the Hub protocol, shared by engines and GUIs.
//!
//! Every line is a command followed by `name=value` arguments. Values that
//! contain spaces or `=` are double-quoted, and an argument may be a bare
//! flag without a value.
//!
//! ```
//! use dame::hub::{Line, quote};
//!
//! let line = Line::parse(r#"info depth=12 pv="32-28 19-23""#).unwrap();
//! assert_eq!(line.command, "info");
//! assert_eq!(line.get("pv"), Some("32-28 19-23"));
//! assert_eq!(quote("32-28 19-23"), r#""32-28 19-23""#);
//! ```
//!
//! Specification: <https://github.com/rhalbersma/scan/blob/master/protocol.txt>.

/// A parsed protocol line: the command and its arguments.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Line {
    /// The command, e.g. `pos` or `info`.
    pub command: String,
    /// The `name=value` arguments in order; flags have an empty value.
    pub args: Vec<(String, String)>,
}

impl Line {
    /// Splits `text` into a command and `name[=value]` arguments, honouring
    /// double quotes. Returns `None` for blank lines.
    pub fn parse(text: &str) -> Option<Self> {
        let mut tokens = Vec::new();
        let mut token = String::new();
        let mut quoted = false;
        let mut started = false;
        for c in text.chars() {
            match c {
                '"' => {
                    quoted = !quoted;
                    started = true;
                }
                c if c.is_whitespace() && !quoted => {
                    if started {
                        tokens.push(std::mem::take(&mut token));
                        started = false;
                    }
                }
                c => {
                    token.push(c);
                    started = true;
                }
            }
        }
        if started {
            tokens.push(token);
        }
        let mut tokens = tokens.into_iter();
        let command = tokens.next()?;
        let args = tokens
            .map(|token| match token.split_once('=') {
                Some((name, value)) => (name.to_owned(), value.to_owned()),
                None => (token, String::new()),
            })
            .collect();
        Some(Self { command, args })
    }

    /// The value of the first argument called `name`.
    pub fn get(&self, name: &str) -> Option<&str> {
        self.args
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Whether the line has an argument or flag called `name`.
    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// Quotes `value` if the protocol requires it. Double quotes cannot be
/// escaped, so they are dropped.
pub fn quote(value: &str) -> String {
    if value.is_empty() || value.contains([' ', '=', '"']) {
        format!("\"{}\"", value.replace('"', ""))
    } else {
        value.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_lines() {
        let line = Line::parse(r#"pos pos=Wbbb moves="32-28 19-23" flag"#).unwrap();
        assert_eq!(line.command, "pos");
        assert_eq!(line.get("pos"), Some("Wbbb"));
        assert_eq!(line.get("moves"), Some("32-28 19-23"));
        assert!(line.has("flag"));
        assert!(!line.has("missing"));
        assert!(Line::parse("   ").is_none());
        assert_eq!(Line::parse(r#"x a="""#).unwrap().get("a"), Some(""));
    }

    #[test]
    fn quoting() {
        assert_eq!(quote("abc"), "abc");
        assert_eq!(quote(""), "\"\"");
        assert_eq!(quote("a b"), "\"a b\"");
    }
}
