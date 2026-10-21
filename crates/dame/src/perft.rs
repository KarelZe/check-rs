//! Move-path enumeration ("perft") for validating the move generator.

use crate::moves::Move;
use crate::position::Position;

/// Counts the leaf nodes of the legal move tree of `pos` at `depth` plies.
///
/// Duplicate capture sequences are not counted twice, so the results match
/// the published perft numbers with duplicates removed.
pub fn perft(pos: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = pos.legal_moves();
    if depth == 1 {
        return moves.len() as u64;
    }
    moves
        .iter()
        .map(|&mv| perft(&pos.after(mv), depth - 1))
        .sum()
}

/// Splits [`perft`] by root move, which helps to locate generator bugs.
pub fn divide(pos: &Position, depth: u32) -> Vec<(Move, u64)> {
    let depth = depth.max(1);
    pos.legal_moves()
        .iter()
        .map(|&mv| (mv, perft(&pos.after(mv), depth - 1)))
        .collect()
}
