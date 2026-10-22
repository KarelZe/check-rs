//! Parsing moves from PDN and Hub notation.

use std::fmt;

use crate::bitboard::Bitboard;
use crate::moves::Move;
use crate::position::Position;
use crate::square::Square;

/// The error returned when move text cannot be matched to a legal move.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseMoveError {
    /// The text is not a move in PDN or Hub notation.
    Syntax(String),
    /// No legal move matches the text.
    Illegal(String),
    /// Several legal moves match; add the captured squares to disambiguate.
    Ambiguous(String),
}

impl fmt::Display for ParseMoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Syntax(s) => write!(f, "invalid move `{s}`: expected e.g. `32-28` or `28x19`"),
            Self::Illegal(s) => write!(f, "illegal move `{s}`"),
            Self::Ambiguous(s) => write!(
                f,
                "ambiguous move `{s}`: list the captured squares, e.g. `28x10x14x23`"
            ),
        }
    }
}

impl std::error::Error for ParseMoveError {}

impl Position {
    /// Matches move text against the legal moves of this position.
    ///
    /// Accepted forms:
    ///
    /// * `32-28` — a quiet move;
    /// * `28x19` — a capture, from origin to destination;
    /// * `28x19x23...` — Hub notation: origin, destination, then the
    ///   captured squares in any order;
    /// * `28x19x10` — a PDN capture path through intermediate landing
    ///   squares, accepted when origin and destination are unambiguous.
    pub fn parse_move(&self, text: &str) -> Result<Move, ParseMoveError> {
        let text = text.trim();
        let syntax = || ParseMoveError::Syntax(text.to_owned());
        let is_capture = text.contains('x');
        let squares = text
            .split(['-', 'x'])
            .map(str::parse::<Square>)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| syntax())?;
        if squares.len() < 2
            || (!is_capture && squares.len() != 2)
            || (is_capture && text.contains('-'))
        {
            return Err(syntax());
        }

        let legal = self.legal_moves();
        let candidates = legal
            .iter()
            .filter(|mv| mv.from() == squares[0] && mv.is_capture() == is_capture);
        let unique = |mut matches: Vec<Move>| match matches.len() {
            0 => Err(ParseMoveError::Illegal(text.to_owned())),
            1 => Ok(matches.pop().expect("one match")),
            _ => Err(ParseMoveError::Ambiguous(text.to_owned())),
        };

        if squares.len() > 2 {
            let captured: Bitboard = squares[2..].iter().copied().collect();
            let hub: Vec<Move> = candidates
                .clone()
                .filter(|mv| mv.to() == squares[1] && mv.captured() == captured)
                .copied()
                .collect();
            if !hub.is_empty() {
                return unique(hub);
            }
        }
        let last = *squares.last().expect("at least two squares");
        unique(candidates.filter(|mv| mv.to() == last).copied().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quiet_and_simple_captures() {
        let pos = Position::initial();
        assert_eq!(pos.parse_move("32-28").unwrap().to_string(), "32-28");
        assert!(matches!(
            pos.parse_move("32-27x"),
            Err(ParseMoveError::Syntax(_))
        ));
        assert!(matches!(
            pos.parse_move("32-23"),
            Err(ParseMoveError::Illegal(_))
        ));
        assert!(matches!(
            pos.parse_move("32x23"),
            Err(ParseMoveError::Illegal(_))
        ));
        assert!(matches!(
            pos.parse_move("hello"),
            Err(ParseMoveError::Syntax(_))
        ));
    }

    #[test]
    fn capture_forms() {
        // 32x12 takes 27 and 17, landing on 21 in between.
        let pos = Position::from_fen("W:W28,32:B17,23,27").unwrap();
        let mv = pos.parse_move("32x12").unwrap();
        assert_eq!(pos.parse_move("32x12x17x27").unwrap(), mv);
        assert_eq!(pos.parse_move("32x12x27x17").unwrap(), mv);
        assert_eq!(pos.parse_move("32x21x12").unwrap(), mv);
    }

    #[test]
    fn ambiguous_captures_need_captured_squares() {
        // The king circles the diamond in either direction back to 32 or
        // stops behind 28 or 27; ending on 32 is one move.
        let pos = Position::from_fen("W:WK32:B17,18,27,28").unwrap();
        assert!(pos.parse_move("32x32").is_ok());
        let legal = pos.legal_moves();
        for mv in &legal {
            assert_eq!(pos.parse_move(&mv.to_hub()).unwrap(), *mv);
        }
    }
}
