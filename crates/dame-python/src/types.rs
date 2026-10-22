//! Small value types: colors, pieces, moves and outcomes.

use pyo3::prelude::*;

/// A side. White moves first.
#[pyclass(eq, eq_int, frozen, hash, from_py_object, module = "dame")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Color {
    #[pyo3(name = "WHITE")]
    White,
    #[pyo3(name = "BLACK")]
    Black,
}

#[pymethods]
impl Color {
    /// The other side: `~Color.WHITE` is `Color.BLACK`.
    fn __invert__(&self) -> Self {
        Self::from(!dame::Color::from(*self))
    }

    fn __str__(&self) -> String {
        dame::Color::from(*self).to_string()
    }
}

impl From<dame::Color> for Color {
    fn from(color: dame::Color) -> Self {
        match color {
            dame::Color::White => Self::White,
            dame::Color::Black => Self::Black,
        }
    }
}

impl From<Color> for dame::Color {
    fn from(color: Color) -> Self {
        match color {
            Color::White => Self::White,
            Color::Black => Self::Black,
        }
    }
}

/// A piece: a color and whether it is a king.
#[pyclass(eq, frozen, hash, module = "dame")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Piece(pub(crate) dame::Piece);

#[pymethods]
impl Piece {
    #[new]
    #[pyo3(signature = (color, *, king = false))]
    fn new(color: Color, king: bool) -> Self {
        Self(dame::Piece {
            color: color.into(),
            kind: if king {
                dame::PieceKind::King
            } else {
                dame::PieceKind::Man
            },
        })
    }

    /// The owner of the piece.
    #[getter]
    fn color(&self) -> Color {
        self.0.color.into()
    }

    /// Whether the piece is a king.
    #[getter]
    fn is_king(&self) -> bool {
        self.0.kind == dame::PieceKind::King
    }

    /// `w`/`b` for men, `W`/`B` for kings.
    fn symbol(&self) -> char {
        self.0.to_char()
    }

    fn __str__(&self) -> char {
        self.0.to_char()
    }

    fn __repr__(&self) -> String {
        let color = if self.0.color == dame::Color::White {
            "WHITE"
        } else {
            "BLACK"
        };
        format!(
            "Piece(Color.{color}, king={})",
            if self.is_king() { "True" } else { "False" }
        )
    }
}

/// A legal move: origin, destination and the captured squares.
#[pyclass(eq, frozen, hash, from_py_object, module = "dame")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Move(pub(crate) dame::Move);

#[pymethods]
impl Move {
    /// The origin square, 1 to 50.
    #[getter]
    fn from_square(&self) -> u8 {
        self.0.from().number()
    }

    /// The destination square, 1 to 50.
    #[getter]
    fn to_square(&self) -> u8 {
        self.0.to().number()
    }

    /// The squares of the captured pieces in ascending order.
    #[getter]
    fn captured(&self) -> Vec<u32> {
        // Not `Vec<u8>`: PyO3 converts byte vectors to `bytes`.
        self.0
            .captured()
            .iter()
            .map(|sq| u32::from(sq.number()))
            .collect()
    }

    /// Whether the move captures.
    #[getter]
    fn is_capture(&self) -> bool {
        self.0.is_capture()
    }

    /// The move in Hub notation, which lists the captured squares.
    fn hub(&self) -> String {
        self.0.to_hub()
    }

    fn __str__(&self) -> String {
        self.0.to_string()
    }

    fn __repr__(&self) -> String {
        format!("Move('{}')", self.0.to_hub())
    }
}

/// Why a game ended.
#[pyclass(eq, eq_int, frozen, hash, module = "dame")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum Termination {
    #[pyo3(name = "NO_MOVES")]
    NoMoves,
    #[pyo3(name = "REPETITION")]
    Repetition,
    #[pyo3(name = "TWENTY_FIVE_MOVES")]
    TwentyFiveMoves,
    #[pyo3(name = "SIXTEEN_MOVES")]
    SixteenMoves,
    #[pyo3(name = "FIVE_MOVES")]
    FiveMoves,
}

impl From<dame::Termination> for Termination {
    fn from(termination: dame::Termination) -> Self {
        match termination {
            dame::Termination::NoMoves => Self::NoMoves,
            dame::Termination::Repetition => Self::Repetition,
            dame::Termination::TwentyFiveMoves => Self::TwentyFiveMoves,
            dame::Termination::SixteenMoves => Self::SixteenMoves,
            dame::Termination::FiveMoves => Self::FiveMoves,
        }
    }
}

/// The result of a finished game.
#[pyclass(eq, frozen, hash, module = "dame")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Outcome(pub(crate) dame::Outcome);

#[pymethods]
impl Outcome {
    /// The winner, or `None` for a draw.
    #[getter]
    fn winner(&self) -> Option<Color> {
        self.0.winner.map(Color::from)
    }

    /// The rule that ended the game.
    #[getter]
    fn termination(&self) -> Termination {
        self.0.termination.into()
    }

    /// The PDN result: `"2-0"`, `"0-2"` or `"1-1"`.
    fn result(&self) -> &'static str {
        self.0.result()
    }

    fn __repr__(&self) -> String {
        let winner = match self.0.winner {
            Some(dame::Color::White) => "Color.WHITE",
            Some(dame::Color::Black) => "Color.BLACK",
            None => "None",
        };
        let termination = match self.0.termination {
            dame::Termination::NoMoves => "NO_MOVES",
            dame::Termination::Repetition => "REPETITION",
            dame::Termination::TwentyFiveMoves => "TWENTY_FIVE_MOVES",
            dame::Termination::SixteenMoves => "SIXTEEN_MOVES",
            dame::Termination::FiveMoves => "FIVE_MOVES",
        };
        format!("Outcome(winner={winner}, termination=Termination.{termination})")
    }
}
