//! Zobrist keys for incremental position hashing.
//!
//! The keys are generated at compile time from a fixed seed, so hashes are
//! stable across runs, platforms and releases with the same key schedule.
//! Opening books and other persisted tables may rely on that.

use crate::piece::{Color, Piece, PieceKind};
use crate::square::Square;

const fn splitmix64(state: u64) -> (u64, u64) {
    let state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    (state, z ^ (z >> 31))
}

const SEED: u64 = 0x6461_6D65; // "dame"

/// One key per piece type and bit index; ghost entries are never used.
static PIECE_KEYS: [[u64; 64]; 4] = {
    let mut keys = [[0; 64]; 4];
    let mut state = SEED;
    let mut piece = 0;
    while piece < 4 {
        let mut bit = 0;
        while bit < 64 {
            let (next, key) = splitmix64(state);
            state = next;
            keys[piece][bit] = key;
            bit += 1;
        }
        piece += 1;
    }
    keys
};

/// Toggled whenever the side to move changes; set when Black is to move.
pub(crate) const SIDE_KEY: u64 = splitmix64(SEED ^ 0xFFFF_FFFF_FFFF_FFFF).1;

#[inline(always)]
const fn index(color: Color, kind: PieceKind) -> usize {
    match (color, kind) {
        (Color::White, PieceKind::Man) => 0,
        (Color::Black, PieceKind::Man) => 1,
        (Color::White, PieceKind::King) => 2,
        (Color::Black, PieceKind::King) => 3,
    }
}

/// The key of `piece` standing on `square`.
#[inline(always)]
pub(crate) fn piece_key(piece: Piece, square: Square) -> u64 {
    PIECE_KEYS[index(piece.color, piece.kind)][square.bit() as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_are_distinct() {
        let mut all: Vec<u64> = PIECE_KEYS.iter().flatten().copied().collect();
        all.push(SIDE_KEY);
        let n = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), n);
    }
}
