//! 64-bit bitboards over the 50 playable squares.
//!
//! # Layout
//!
//! Squares are numbered 1..=50 as in standard notation: square 1 is the
//! top-left playable square seen from White, which plays upwards from
//! squares 31..=50. Square `s` maps to bit `d + d / 10` with `d = s - 1`, so
//! every pair of rows occupies 11 bits: five squares of an even row, five
//! squares of an odd row and one unused "ghost" bit (bits 10, 21, 32 and 43).
//!
//! The ghost bits make every diagonal step a fixed shift:
//!
//! | direction  | shift |
//! |------------|-------|
//! | up-left    | -6    |
//! | up-right   | -5    |
//! | down-left  | +5    |
//! | down-right | +6    |
//!
//! A step off the left or right edge lands on a ghost bit and a step off the
//! top or bottom edge falls out of the 54-bit range, so masking with
//! [`Bitboard::ALL`] (or with an `empty` set derived from it) handles the
//! board edges without per-row masks. Ed Gilbert's endgame database driver
//! uses the same layout.

use std::fmt;
use std::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Not};

use crate::square::Square;

/// A set of squares, one bit per square in the ghost-square layout.
#[derive(Copy, Clone, Default, PartialEq, Eq, Hash)]
#[repr(transparent)]
pub struct Bitboard(pub(crate) u64);

impl Bitboard {
    /// The empty set.
    pub const EMPTY: Self = Self(0);

    /// All 50 playable squares.
    pub const ALL: Self = Self(0x003F_F7FE_FFDF_FBFF);

    /// Squares 1..=5, where White promotes.
    pub const WHITE_PROMOTION: Self = Self(0x1F);

    /// Squares 46..=50, where Black promotes.
    pub const BLACK_PROMOTION: Self = Self(0x1F << 49);

    /// Builds a bitboard from raw bits in the ghost-square layout.
    ///
    /// Returns `None` if any ghost or out-of-range bit is set.
    #[inline]
    pub const fn from_bits(bits: u64) -> Option<Self> {
        if bits & !Self::ALL.0 == 0 {
            Some(Self(bits))
        } else {
            None
        }
    }

    /// The raw bits in the ghost-square layout.
    #[inline]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// The number of squares in the set.
    #[inline]
    pub const fn count(self) -> u32 {
        self.0.count_ones()
    }

    /// Whether the set is empty.
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Whether `square` is in the set.
    #[inline]
    pub const fn contains(self, square: Square) -> bool {
        self.0 & square.bitboard().0 != 0
    }

    /// The lowest-numbered square in the set, if any.
    #[inline]
    pub const fn first(self) -> Option<Square> {
        if self.0 == 0 {
            None
        } else {
            Some(Square::from_bit(self.0.trailing_zeros() as u8))
        }
    }

    /// Shifts every square one step in direction `dir` (one of -6, -5, 5, 6).
    ///
    /// Squares stepping off the board land on ghost or out-of-range bits;
    /// callers mask the result.
    #[inline(always)]
    pub(crate) const fn shift(self, dir: i32) -> Self {
        if dir > 0 {
            Self(self.0 << dir)
        } else {
            Self(self.0 >> -dir)
        }
    }

    /// Iterates over the squares in the set, lowest first.
    #[inline]
    pub const fn iter(self) -> Squares {
        Squares(self.0)
    }
}

impl fmt::Debug for Bitboard {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set()
            .entries(self.iter().map(Square::number))
            .finish()
    }
}

impl IntoIterator for Bitboard {
    type Item = Square;
    type IntoIter = Squares;

    #[inline]
    fn into_iter(self) -> Squares {
        self.iter()
    }
}

impl FromIterator<Square> for Bitboard {
    fn from_iter<I: IntoIterator<Item = Square>>(iter: I) -> Self {
        iter.into_iter()
            .fold(Self::EMPTY, |bb, sq| bb | sq.bitboard())
    }
}

/// Iterator over the squares of a [`Bitboard`], lowest first.
#[derive(Clone, Debug)]
pub struct Squares(u64);

impl Iterator for Squares {
    type Item = Square;

    #[inline]
    fn next(&mut self) -> Option<Square> {
        if self.0 == 0 {
            return None;
        }
        let bit = self.0.trailing_zeros() as u8;
        self.0 &= self.0 - 1;
        Some(Square::from_bit(bit))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.count_ones() as usize;
        (n, Some(n))
    }
}

impl ExactSizeIterator for Squares {}

impl Not for Bitboard {
    type Output = Self;

    /// The complement within the playable squares.
    #[inline]
    fn not(self) -> Self {
        Self(!self.0 & Self::ALL.0)
    }
}

macro_rules! impl_bit_op {
    ($trait:ident, $fn:ident, $assign_trait:ident, $assign_fn:ident, $op:tt) => {
        impl $trait for Bitboard {
            type Output = Self;

            #[inline]
            fn $fn(self, rhs: Self) -> Self {
                Self(self.0 $op rhs.0)
            }
        }

        impl $assign_trait for Bitboard {
            #[inline]
            fn $assign_fn(&mut self, rhs: Self) {
                self.0 = self.0 $op rhs.0;
            }
        }
    };
}

impl_bit_op!(BitAnd, bitand, BitAndAssign, bitand_assign, &);
impl_bit_op!(BitOr, bitor, BitOrAssign, bitor_assign, |);
impl_bit_op!(BitXor, bitxor, BitXorAssign, bitxor_assign, ^);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_has_fifty_squares_and_no_ghosts() {
        assert_eq!(Bitboard::ALL.count(), 50);
        for ghost in [10, 21, 32, 43] {
            assert_eq!(Bitboard::ALL.0 & (1 << ghost), 0);
        }
        assert_eq!(Bitboard::ALL.0 >> 54, 0);
    }

    #[test]
    fn promotion_rows() {
        let white: Vec<u8> = Bitboard::WHITE_PROMOTION
            .iter()
            .map(Square::number)
            .collect();
        let black: Vec<u8> = Bitboard::BLACK_PROMOTION
            .iter()
            .map(Square::number)
            .collect();
        assert_eq!(white, [1, 2, 3, 4, 5]);
        assert_eq!(black, [46, 47, 48, 49, 50]);
    }

    /// Every shift either reaches the true diagonal neighbour or leaves the board.
    #[test]
    fn shifts_match_geometric_neighbours() {
        for sq in Bitboard::ALL {
            let (row, file) = (sq.row() as i32, sq.file() as i32);
            for (dir, dr, df) in [(-6, -1, -1), (-5, -1, 1), (5, 1, -1), (6, 1, 1)] {
                let shifted = sq.bitboard().shift(dir) & Bitboard::ALL;
                let expected = Bitboard::ALL
                    .iter()
                    .find(|t| t.row() as i32 == row + dr && t.file() as i32 == file + df);
                assert_eq!(shifted.first(), expected, "square {sq}, dir {dir}");
            }
        }
    }

    #[test]
    fn complement_stays_on_board() {
        assert_eq!(!Bitboard::EMPTY, Bitboard::ALL);
        assert_eq!(!Bitboard::ALL, Bitboard::EMPTY);
    }

    #[test]
    fn from_bits_rejects_ghosts() {
        assert!(Bitboard::from_bits(1 << 10).is_none());
        assert!(Bitboard::from_bits(1 << 60).is_none());
        assert_eq!(Bitboard::from_bits(1), Some(Bitboard(1)));
    }
}
