//! Playable squares, numbered 1..=50 in standard notation.

use std::fmt;
use std::str::FromStr;

use crate::bitboard::Bitboard;

/// One of the 50 playable squares.
///
/// Internally a square is its bit index in the ghost-square layout (see
/// [`crate::bitboard`]); [`Square::number`] gives the standard 1..=50 number.
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Square(u8);

impl Square {
    /// Builds a square from its standard number in `1..=50`.
    #[inline]
    pub const fn new(number: u8) -> Option<Self> {
        if number >= 1 && number <= 50 {
            let d = number - 1;
            Some(Self(d + d / 10))
        } else {
            None
        }
    }

    /// Builds a square from a bit index that is known to be playable.
    #[inline]
    pub(crate) const fn from_bit(bit: u8) -> Self {
        debug_assert!(bit < 54 && bit % 11 != 10);
        Self(bit)
    }

    /// The standard square number in `1..=50`.
    #[inline]
    pub const fn number(self) -> u8 {
        self.0 - self.0 / 11 + 1
    }

    /// The bit index in the ghost-square layout.
    #[inline]
    pub const fn bit(self) -> u8 {
        self.0
    }

    /// The single-square bitboard.
    #[inline]
    pub const fn bitboard(self) -> Bitboard {
        Bitboard(1 << self.0)
    }

    /// The row from White's point of view: 0 is the top row (squares 1..=5),
    /// 9 the bottom row (squares 46..=50).
    #[inline]
    pub const fn row(self) -> u8 {
        (self.number() - 1) / 5
    }

    /// The file from White's point of view: 0 is the left edge (file `a`),
    /// 9 the right edge (file `j`).
    #[inline]
    pub const fn file(self) -> u8 {
        let d = self.number() - 1;
        2 * (d % 5) + 1 - (d / 5) % 2
    }

    /// Iterates over all 50 squares in numeric order.
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        Bitboard::ALL.iter()
    }
}

impl fmt::Debug for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Square({})", self.number())
    }
}

impl fmt::Display for Square {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.number(), f)
    }
}

/// The error returned when parsing a square number fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseSquareError(pub(crate) String);

impl fmt::Display for ParseSquareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "invalid square `{}`: expected a number from 1 to 50",
            self.0
        )
    }
}

impl std::error::Error for ParseSquareError {}

impl FromStr for Square {
    type Err = ParseSquareError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.parse::<u8>()
            .ok()
            .and_then(Self::new)
            .ok_or_else(|| ParseSquareError(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_round_trips() {
        for n in 1..=50 {
            assert_eq!(Square::new(n).unwrap().number(), n);
        }
        assert!(Square::new(0).is_none());
        assert!(Square::new(51).is_none());
    }

    #[test]
    fn corners() {
        let at = |n| {
            let sq = Square::new(n).unwrap();
            (sq.row(), sq.file())
        };
        assert_eq!(at(1), (0, 1));
        assert_eq!(at(5), (0, 9));
        assert_eq!(at(6), (1, 0));
        assert_eq!(at(46), (9, 0));
        assert_eq!(at(50), (9, 8));
    }

    #[test]
    fn parse() {
        assert_eq!("32".parse::<Square>().unwrap().number(), 32);
        assert!("0".parse::<Square>().is_err());
        assert!("x".parse::<Square>().is_err());
    }
}
