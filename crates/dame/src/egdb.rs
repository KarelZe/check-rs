//! Endgame database interface.
//!
//! The search probes an [`EndgameDb`] in positions with few pieces. No
//! database ships with the crate yet; the trait is the integration point for
//! Ed Gilbert's international draughts databases, whose access code
//! (<https://github.com/eygilbert/egdb_intl>, Boost Software License 1.0)
//! uses the same ghost-square bitboard layout as this crate.

use crate::position::Position;

/// A game-theoretic value from the side to move's point of view.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Wdl {
    /// The side to move wins with best play.
    Win,
    /// Neither side can force a win.
    Draw,
    /// The side to move loses with best play.
    Loss,
}

/// A source of perfect endgame knowledge.
///
/// Implementations must be thread-safe: future multi-threaded searches share
/// one database between threads.
pub trait EndgameDb: Send + Sync {
    /// The largest number of pieces on the board the database covers.
    fn max_pieces(&self) -> u32;

    /// The value of `pos`, or `None` if the database cannot answer.
    ///
    /// The search only probes positions with at most [`Self::max_pieces`]
    /// pieces in which the side to move has no capture. Implementations
    /// must return `None` for positions their tables do not resolve, such
    /// as positions excluded from compression.
    fn probe(&self, pos: &Position) -> Option<Wdl>;
}
