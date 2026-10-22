//! Python bindings for the dame draughts engine.
//!
//! The extension is imported as `dame._core`; the public, documented API
//! is re-exported from the pure-Python `dame` package.

// PyO3 dictates the receiver and argument types of exported methods, and
// method names follow Python conventions (`Move.from_square`).
#![allow(
    clippy::needless_pass_by_value,
    clippy::trivially_copy_pass_by_ref,
    clippy::wrong_self_convention
)]

mod board;
mod engine;
mod types;

use pyo3::create_exception;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

create_exception!(
    dame,
    IllegalMoveError,
    PyValueError,
    "Raised when a move is illegal or cannot be parsed."
);

/// Counts the leaf nodes of the legal move tree at `depth` plies.
#[pyfunction]
fn perft(py: Python<'_>, board: &board::Board, depth: u32) -> u64 {
    let position = *board.game.position();
    py.detach(|| dame::perft(&position, depth))
}

#[pymodule]
mod _core {
    #[pymodule_export]
    use super::IllegalMoveError;
    #[pymodule_export]
    use super::board::Board;
    #[pymodule_export]
    use super::engine::{Engine, OpeningBook, SearchInfo, SearchResult};
    #[pymodule_export]
    use super::perft;
    #[pymodule_export]
    use super::types::{Color, Move, Outcome, Piece, Termination};
    use pyo3::prelude::*;

    #[pymodule_init]
    fn init(m: &Bound<'_, PyModule>) -> PyResult<()> {
        m.add("__version__", dame::VERSION)?;
        m.add("STARTING_FEN", dame::START_FEN)?;
        m.add("WHITE", super::types::Color::White)?;
        m.add("BLACK", super::types::Color::Black)?;
        Ok(())
    }
}
