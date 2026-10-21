//! A fast international draughts (10x10 checkers) engine.
//!
//! The crate is the engine core: board representation, move generation,
//! search and evaluation. It has no dependencies and no Python coupling; the
//! `dame-cli` and `dame-python` crates build on top of it.
//!
//! ```
//! use dame::{Position, perft};
//!
//! let pos = Position::initial();
//! let moves = pos.legal_moves();
//! assert_eq!(moves.len(), 9);
//!
//! assert_eq!(perft(&pos, 2), 81);
//!
//! let next = pos.after(moves[0]);
//! assert_eq!(next.legal_moves().len(), 9);
//! ```

#![warn(missing_docs)]

pub mod bitboard;
mod movegen;
pub mod moves;
mod perft;
pub mod piece;
pub mod position;
pub mod square;
mod zobrist;

pub use bitboard::Bitboard;
pub use moves::{Move, MoveList};
pub use perft::{divide, perft};
pub use piece::{Color, Piece, PieceKind};
pub use position::{Position, PositionError};
pub use square::Square;

/// The crate version, as declared in `Cargo.toml`.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
