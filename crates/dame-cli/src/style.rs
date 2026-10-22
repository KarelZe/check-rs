//! Colored terminal output for the commands people read, in the style of
//! ruff, ty and uv. The Hub protocol is for programs and is never colored.
//!
//! Color is on when the output goes to a terminal. The usual environment
//! variables override that:
//!
//! * `NO_COLOR` (<https://no-color.org>) turns it off;
//! * `FORCE_COLOR` (<https://force-color.org>) or `CLICOLOR_FORCE` turn it on,
//!   e.g. for a pager such as `less -R`.
//!
//! `NO_COLOR` wins over the other two, and `TERM=dumb` counts as no terminal.
//! The styles are `anstyle`'s, as re-exported by clap, so coloring needs no
//! extra dependency.

use std::env;
use std::fmt::Display;
use std::io::IsTerminal;

use clap::builder::styling::{AnsiColor, Effects, Style};

/// Moves: the engine's choice, hints, the best move.
pub(crate) const MOVE: Style = AnsiColor::Cyan.on_default().effects(Effects::BOLD);
/// Prompts and headlines.
pub(crate) const BOLD: Style = Style::new().effects(Effects::BOLD);
/// Secondary details: search statistics, square numbers, labels.
pub(crate) const DIM: Style = Style::new().effects(Effects::DIMMED);
/// Errors.
pub(crate) const ERROR: Style = AnsiColor::Red.on_default().effects(Effects::BOLD);
/// Scores in favour of, and against, the side to move.
pub(crate) const GOOD: Style = AnsiColor::Green.on_default();
pub(crate) const BAD: Style = AnsiColor::Red.on_default();
/// White's and Black's pieces on the board.
pub(crate) const WHITE: Style = Style::new().effects(Effects::BOLD);
pub(crate) const BLACK: Style = AnsiColor::Red.on_default().effects(Effects::BOLD);
/// The squares of the last move.
pub(crate) const LAST_MOVE: Style = AnsiColor::Yellow.on_default().effects(Effects::BOLD);

/// Styles text, or leaves it plain when color is off.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) struct Painter {
    color: bool,
}

impl Painter {
    /// A painter that never colors.
    #[cfg(test)]
    pub(crate) const PLAIN: Self = Self { color: false };
    /// A painter that always colors.
    #[cfg(test)]
    pub(crate) const COLOR: Self = Self { color: true };

    /// For standard output.
    pub(crate) fn stdout() -> Self {
        Self::from_env(std::io::stdout().is_terminal())
    }

    /// For standard error.
    pub(crate) fn stderr() -> Self {
        Self::from_env(std::io::stderr().is_terminal())
    }

    fn from_env(terminal: bool) -> Self {
        let set = |name| env::var_os(name).is_some_and(|value| !value.is_empty());
        let forced = |name| env::var(name).is_ok_and(|value| !value.is_empty() && value != "0");
        Self {
            color: decide(
                set("NO_COLOR"),
                forced("FORCE_COLOR") || forced("CLICOLOR_FORCE"),
                terminal && ansi_terminal(),
            ),
        }
    }

    pub(crate) fn is_colored(self) -> bool {
        self.color
    }

    /// `text` in `style`.
    pub(crate) fn paint(self, style: Style, text: impl Display) -> String {
        if self.color {
            format!("{style}{text}{style:#}")
        } else {
            text.to_string()
        }
    }

    /// A score in men, green when it favours the side to move and red when
    /// it does not.
    pub(crate) fn score(self, centi_men: i32, width: usize) -> String {
        let text = format!("{:+width$.2}", f64::from(centi_men) / 100.0);
        match centi_men.signum() {
            1 => self.paint(GOOD, text),
            -1 => self.paint(BAD, text),
            _ => text,
        }
    }
}

/// Whether to color: `NO_COLOR` wins, then a forced color, then whether the
/// output is a terminal that understands ANSI escape codes.
fn decide(no_color: bool, forced: bool, ansi_terminal: bool) -> bool {
    !no_color && (forced || ansi_terminal)
}

/// Whether the terminal understands ANSI escape codes. The classic Windows
/// console only does after a system call that `std` does not make, so on
/// Windows only terminals that announce themselves count: Windows Terminal
/// (`WT_SESSION`) and Unix-like ones (`TERM`, e.g. Git Bash).
fn ansi_terminal() -> bool {
    let term = env::var_os("TERM");
    if term.as_deref().is_some_and(|term| term == "dumb") {
        return false;
    }
    !cfg!(windows) || env::var_os("WT_SESSION").is_some() || term.is_some()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// `text` without ANSI escape codes.
    pub(crate) fn strip(text: &str) -> String {
        let mut plain = String::new();
        let mut chars = text.chars();
        while let Some(c) = chars.next() {
            if c == '\x1b' {
                // An SGR sequence: ESC [ parameters m.
                for c in chars.by_ref() {
                    if c == 'm' {
                        break;
                    }
                }
            } else {
                plain.push(c);
            }
        }
        plain
    }

    #[test]
    fn no_color_wins() {
        assert!(decide(false, false, true));
        assert!(!decide(false, false, false));
        assert!(decide(false, true, false));
        assert!(!decide(true, true, true));
        assert!(!decide(true, false, true));
    }

    #[test]
    fn painting() {
        let colored = Painter::COLOR;
        assert_eq!(Painter::PLAIN.paint(MOVE, "32-28"), "32-28");
        assert_ne!(colored.paint(MOVE, "32-28"), "32-28");
        assert_eq!(strip(&colored.paint(MOVE, "32-28")), "32-28");
        assert_eq!(Painter::PLAIN.score(-125, 6), " -1.25");
        assert_eq!(strip(&colored.score(250, 0)), "+2.50");
        assert_eq!(colored.score(0, 0), "+0.00");
    }
}
