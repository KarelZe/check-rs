//! Moves and fixed-capacity move lists.

use std::fmt;
use std::mem::MaybeUninit;
use std::ops::Deref;

use crate::bitboard::Bitboard;
use crate::square::Square;

/// A legal move: origin, destination and the set of captured pieces.
///
/// Two capture sequences with the same origin, destination and captured set
/// are the same move under the rules, so the landing squares in between are
/// not stored. A king capture may end on its own origin, so `from == to` is
/// valid when [`Move::captured`] is non-empty.
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
pub struct Move {
    captured: Bitboard,
    from: Square,
    to: Square,
}

impl Move {
    #[inline]
    pub(crate) const fn new(from: Square, to: Square, captured: Bitboard) -> Self {
        Self { captured, from, to }
    }

    /// The square the piece moves from.
    #[inline]
    pub const fn from(self) -> Square {
        self.from
    }

    /// The square the piece moves to.
    #[inline]
    pub const fn to(self) -> Square {
        self.to
    }

    /// The squares of the captured pieces; empty for a quiet move.
    #[inline]
    pub const fn captured(self) -> Bitboard {
        self.captured
    }

    /// Whether the move captures at least one piece.
    #[inline]
    pub const fn is_capture(self) -> bool {
        !self.captured.is_empty()
    }

    /// The move in Hub protocol notation: `32-28` for a quiet move and
    /// `from x to x captured...` (e.g. `28x19x23`) for a capture, which is
    /// unambiguous even when two routes share their endpoints.
    pub fn to_hub(self) -> String {
        use fmt::Write;

        if !self.is_capture() {
            return self.to_string();
        }
        let mut s = format!("{}x{}", self.from, self.to);
        for sq in self.captured {
            write!(s, "x{sq}").expect("writing to a String cannot fail");
        }
        s
    }
}

impl fmt::Display for Move {
    /// Short PDN notation: `32-28` for a quiet move, `28x19` for a capture.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sep = if self.is_capture() { 'x' } else { '-' };
        write!(f, "{}{sep}{}", self.from, self.to)
    }
}

impl fmt::Debug for Move {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Move({})", self.to_hub())
    }
}

/// The maximum number of moves a [`MoveList`] holds.
///
/// Quiet moves are bounded by 4 × 48 = 192: every quiet move ends on an
/// empty square, and from each of the four directions at most one piece can
/// reach a given square. After duplicate removal capture lists stay below
/// 128 entries in practice.
pub const MAX_MOVES: usize = 256;

/// A stack-allocated list of moves.
///
/// Dereferences to `[Move]`, so slices methods and iteration work directly.
pub struct MoveList {
    len: usize,
    moves: [MaybeUninit<Move>; MAX_MOVES],
}

impl MoveList {
    /// An empty list.
    #[inline]
    pub const fn new() -> Self {
        Self {
            len: 0,
            moves: [MaybeUninit::uninit(); MAX_MOVES],
        }
    }

    /// Appends a move.
    ///
    /// # Panics
    ///
    /// Panics if the list already holds [`MAX_MOVES`] moves.
    #[inline]
    pub fn push(&mut self, mv: Move) {
        self.moves[self.len].write(mv);
        self.len += 1;
    }

    /// Removes all moves.
    #[inline]
    pub fn clear(&mut self) {
        self.len = 0;
    }

    /// The moves as a mutable slice, e.g. for sorting.
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [Move] {
        // SAFETY: the first `len` elements are initialized by `push`, and
        // `MaybeUninit<Move>` has the same layout as `Move`.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts_mut(self.moves.as_mut_ptr().cast::<Move>(), self.len)
        }
    }
}

impl Default for MoveList {
    fn default() -> Self {
        Self::new()
    }
}

impl Deref for MoveList {
    type Target = [Move];

    #[inline]
    fn deref(&self) -> &[Move] {
        // SAFETY: the first `len` elements are initialized by `push`, and
        // `MaybeUninit<Move>` has the same layout as `Move`.
        #[allow(unsafe_code)]
        unsafe {
            std::slice::from_raw_parts(self.moves.as_ptr().cast::<Move>(), self.len)
        }
    }
}

impl<'a> IntoIterator for &'a MoveList {
    type Item = &'a Move;
    type IntoIter = std::slice::Iter<'a, Move>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Debug for MoveList {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sq(n: u8) -> Square {
        Square::new(n).unwrap()
    }

    #[test]
    fn notation() {
        let quiet = Move::new(sq(32), sq(28), Bitboard::EMPTY);
        assert_eq!(quiet.to_string(), "32-28");
        assert_eq!(quiet.to_hub(), "32-28");

        let captured = sq(23).bitboard() | sq(14).bitboard();
        let capture = Move::new(sq(28), sq(10), captured);
        assert_eq!(capture.to_string(), "28x10");
        assert_eq!(capture.to_hub(), "28x10x14x23");
    }

    #[test]
    fn list_push_and_clear() {
        let mut list = MoveList::new();
        assert!(list.is_empty());
        list.push(Move::new(sq(32), sq(28), Bitboard::EMPTY));
        list.push(Move::new(sq(33), sq(28), Bitboard::EMPTY));
        assert_eq!(list.len(), 2);
        assert_eq!(list[1].from(), sq(33));
        list.clear();
        assert!(list.is_empty());
    }
}
