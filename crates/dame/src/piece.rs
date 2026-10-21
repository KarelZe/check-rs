//! Colors and pieces.

use std::fmt;
use std::ops::Not;

/// A side. White moves first and plays up the board, from squares 31..=50
/// towards squares 1..=5.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Color {
    /// The side that starts on squares 31..=50 and moves first.
    White,
    /// The side that starts on squares 1..=20.
    Black,
}

impl Color {
    /// The FEN letter of the side: `W` or `B`.
    pub const fn to_char(self) -> char {
        match self {
            Self::White => 'W',
            Self::Black => 'B',
        }
    }
}

impl Not for Color {
    type Output = Self;

    #[inline]
    fn not(self) -> Self {
        match self {
            Self::White => Self::Black,
            Self::Black => Self::White,
        }
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::White => "white",
            Self::Black => "black",
        })
    }
}

/// The kind of a piece.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum PieceKind {
    /// A man moves one square diagonally forward and captures in all four
    /// directions.
    Man,
    /// A flying king moves and captures along whole diagonals.
    King,
}

/// A piece: a color and a kind.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Piece {
    /// The owner of the piece.
    pub color: Color,
    /// Whether the piece is a man or a king.
    pub kind: PieceKind,
}

impl Piece {
    /// The character used for the piece in board diagrams and Hub positions:
    /// `w`/`b` for men, `W`/`B` for kings.
    pub const fn to_char(self) -> char {
        match (self.color, self.kind) {
            (Color::White, PieceKind::Man) => 'w',
            (Color::Black, PieceKind::Man) => 'b',
            (Color::White, PieceKind::King) => 'W',
            (Color::Black, PieceKind::King) => 'B',
        }
    }

    /// The inverse of [`Piece::to_char`].
    pub const fn from_char(c: char) -> Option<Self> {
        let (color, kind) = match c {
            'w' => (Color::White, PieceKind::Man),
            'b' => (Color::Black, PieceKind::Man),
            'W' => (Color::White, PieceKind::King),
            'B' => (Color::Black, PieceKind::King),
            _ => return None,
        };
        Some(Self { color, kind })
    }
}
