//! The `Board` class: a game in progress.

use std::collections::BTreeMap;

use dame::{Game, Square};
use pyo3::exceptions::{PyIndexError, PyValueError};
use pyo3::prelude::*;

use crate::IllegalMoveError;
use crate::types::{Color, Move, Outcome, Piece};

/// A move given either as a `Move` or as text such as `"32-28"`.
#[derive(FromPyObject)]
pub(crate) enum MoveLike {
    Move(Move),
    Text(String),
}

/// A draughts board: the start position plus the moves played from it.
#[pyclass(module = "dame")]
#[derive(Clone)]
pub(crate) struct Board {
    pub(crate) game: Game,
}

fn square(number: u8) -> PyResult<Square> {
    Square::new(number)
        .ok_or_else(|| PyValueError::new_err(format!("invalid square {number}: expected 1 to 50")))
}

#[pymethods]
impl Board {
    #[new]
    #[pyo3(signature = (fen = None))]
    fn new(fen: Option<&str>) -> PyResult<Self> {
        let game = match fen {
            None => Game::new(),
            Some(fen) => {
                Game::from_fen(fen).map_err(|err| PyValueError::new_err(err.to_string()))?
            }
        };
        Ok(Self { game })
    }

    /// The current position in FEN notation.
    fn fen(&self) -> String {
        self.game.position().fen()
    }

    /// The side to move.
    #[getter]
    fn turn(&self) -> Color {
        self.game.position().side_to_move().into()
    }

    /// The legal moves; empty once the game is over.
    #[getter]
    fn legal_moves(&self) -> Vec<Move> {
        self.game.legal_moves().iter().copied().map(Move).collect()
    }

    /// The moves played so far.
    #[getter]
    fn move_stack(&self) -> Vec<Move> {
        self.game.moves().iter().copied().map(Move).collect()
    }

    /// The number of moves played so far.
    #[getter]
    fn ply(&self) -> usize {
        self.game.moves().len()
    }

    /// Whether the side to move has to capture.
    #[getter]
    fn must_capture(&self) -> bool {
        self.game.position().has_captures()
    }

    /// Matches move text against the legal moves.
    fn parse_move(&self, text: &str) -> PyResult<Move> {
        self.game
            .position()
            .parse_move(text)
            .map(Move)
            .map_err(|err| IllegalMoveError::new_err(err.to_string()))
    }

    /// Whether `move` is legal in the current position.
    fn is_legal(&self, r#move: &Move) -> bool {
        self.game.legal_moves().contains(&r#move.0)
    }

    /// Plays a move and returns it.
    fn push(&mut self, r#move: MoveLike) -> PyResult<Move> {
        let result = match r#move {
            MoveLike::Move(mv) => self.game.play(mv.0).map(|()| mv.0),
            MoveLike::Text(text) => self.game.play_str(&text),
        };
        result
            .map(Move)
            .map_err(|err| IllegalMoveError::new_err(err.to_string()))
    }

    /// Takes back the last move and returns it.
    fn pop(&mut self) -> PyResult<Move> {
        self.game
            .undo()
            .map(Move)
            .ok_or_else(|| PyIndexError::new_err("pop from a board without moves"))
    }

    /// The piece on `square` (1 to 50), if any.
    fn piece_at(&self, square: u8) -> PyResult<Option<Piece>> {
        Ok(self
            .game
            .position()
            .piece_at(self::square(square)?)
            .map(Piece))
    }

    /// All pieces by square.
    fn piece_map(&self) -> BTreeMap<u8, Piece> {
        let pos = self.game.position();
        pos.occupied()
            .iter()
            .filter_map(|sq| pos.piece_at(sq).map(|piece| (sq.number(), Piece(piece))))
            .collect()
    }

    /// The outcome if the game is over, else `None`.
    fn outcome(&self) -> Option<Outcome> {
        self.game.outcome().map(Outcome)
    }

    /// Whether the game is over.
    fn is_game_over(&self) -> bool {
        self.game.is_over()
    }

    /// A 64-bit Zobrist hash of the current position.
    fn zobrist_hash(&self) -> u64 {
        self.game.position().hash()
    }

    /// An independent copy of the board, including the move history.
    fn copy(&self) -> Self {
        self.clone()
    }

    fn __copy__(&self) -> Self {
        self.clone()
    }

    fn __deepcopy__(&self, _memo: &Bound<'_, PyAny>) -> Self {
        self.clone()
    }

    /// Boards are equal if their current positions are equal.
    fn __eq__(&self, other: &Self) -> bool {
        self.game.position() == other.game.position()
    }

    fn __str__(&self) -> String {
        self.game.position().to_string()
    }

    fn __repr__(&self) -> String {
        format!("Board('{}')", self.fen())
    }

    fn __getnewargs__(&self) -> (String,) {
        (self.game.start().fen(),)
    }

    fn __getstate__(&self) -> Vec<String> {
        self.game.moves().iter().map(|mv| mv.to_hub()).collect()
    }

    fn __setstate__(&mut self, moves: Vec<String>) -> PyResult<()> {
        self.game = Game::from_position(*self.game.start());
        for mv in moves {
            self.game
                .play_str(&mv)
                .map_err(|err| IllegalMoveError::new_err(err.to_string()))?;
        }
        Ok(())
    }
}
