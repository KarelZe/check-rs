//! Board geometry and click-to-move input, independent of the UI toolkit.
//!
//! A move is entered by clicking the piece and then its destination. When
//! several captures share both squares, clicking pieces to capture narrows
//! them down, before or after the destination.

use dame::{Bitboard, Move, Square};

/// The square drawn at `row` and `col` (0..10 from the top-left corner of
/// the drawn board), or `None` for a light square or a cell off the board.
/// A flipped board shows Black's side at the bottom.
pub(crate) fn square_at(row: u8, col: u8, flipped: bool) -> Option<Square> {
    if row >= 10 || col >= 10 || (row + col).is_multiple_of(2) {
        return None;
    }
    let (row, col) = if flipped {
        (9 - row, 9 - col)
    } else {
        (row, col)
    };
    Square::new(row * 5 + col / 2 + 1)
}

/// The cell (row, column) where `square` is drawn; the inverse of
/// [`square_at`].
pub(crate) fn cell_of(square: Square, flipped: bool) -> (u8, u8) {
    let (row, col) = (square.row(), square.file());
    if flipped {
        (9 - row, 9 - col)
    } else {
        (row, col)
    }
}

/// A move being entered by clicks.
#[derive(Clone, Debug, Default)]
pub(crate) struct MoveInput {
    from: Option<Square>,
    to: Option<Square>,
    via: Bitboard,
}

impl MoveInput {
    /// Forgets the move entered so far.
    pub(crate) fn clear(&mut self) {
        *self = Self::default();
    }

    /// The selected piece.
    pub(crate) fn from(&self) -> Option<Square> {
        self.from
    }

    /// The pieces clicked to choose between captures.
    pub(crate) fn via(&self) -> Bitboard {
        self.via
    }

    /// Whether a destination was clicked that several captures share.
    pub(crate) fn is_ambiguous(&self) -> bool {
        self.to.is_some()
    }

    /// The legal moves that match the input so far.
    fn candidates<'a>(&self, moves: &'a [Move]) -> impl Iterator<Item = Move> + 'a {
        let (from, to, via) = (self.from, self.to, self.via);
        moves.iter().copied().filter(move |mv| {
            Some(mv.from()) == from
                && to.is_none_or(|to| mv.to() == to)
                && (mv.captured() & via) == via
        })
    }

    /// The squares the selected piece can still move to.
    pub(crate) fn targets(&self, moves: &[Move]) -> Bitboard {
        self.candidates(moves).map(Move::to).collect()
    }

    /// Handles a click on `square` (or off the board, for `None`) and
    /// returns the move once it is unambiguous.
    pub(crate) fn click(&mut self, square: Option<Square>, moves: &[Move]) -> Option<Move> {
        let Some(square) = square else {
            self.clear();
            return None;
        };
        if self.from.is_some() {
            // A destination; a king capture may end on its own square.
            if self.to.is_none() && self.targets(moves).contains(square) {
                self.to = Some(square);
                return self.unique(moves);
            }
            // A piece to capture, to choose between routes.
            let capturable = self
                .candidates(moves)
                .fold(Bitboard::EMPTY, |acc, mv| acc | mv.captured());
            if capturable.contains(square) && !self.via.contains(square) {
                self.via |= square.bitboard();
                return if self.to.is_some() {
                    self.unique(moves)
                } else {
                    None
                };
            }
        }
        // Otherwise select a piece, or deselect.
        let reselect = self.from != Some(square) && moves.iter().any(|mv| mv.from() == square);
        self.clear();
        if reselect {
            self.from = Some(square);
        }
        None
    }

    /// The single matching move, if there is exactly one.
    fn unique(&mut self, moves: &[Move]) -> Option<Move> {
        let mut candidates = self.candidates(moves);
        let first = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        self.clear();
        Some(first)
    }
}

#[cfg(test)]
mod tests {
    use dame::Position;

    use super::*;

    fn sq(number: u8) -> Option<Square> {
        Square::new(number)
    }

    #[test]
    fn geometry_round_trips() {
        for flipped in [false, true] {
            for square in Square::all() {
                let (row, col) = cell_of(square, flipped);
                assert_eq!(square_at(row, col, flipped), Some(square));
            }
        }
        assert_eq!(square_at(0, 1, false), sq(1));
        assert_eq!(square_at(9, 8, false), sq(50));
        assert_eq!(square_at(0, 1, true), sq(50));
        assert_eq!(square_at(0, 0, false), None);
        assert_eq!(square_at(10, 1, false), None);
    }

    #[test]
    fn quiet_move_by_two_clicks() {
        let moves = Position::initial().legal_moves();
        let mut input = MoveInput::default();
        assert_eq!(input.click(sq(32), &moves), None);
        assert_eq!(input.from(), sq(32));
        assert_eq!(
            input.targets(&moves),
            [27, 28].map(|n| sq(n).unwrap()).into_iter().collect()
        );
        let mv = input.click(sq(28), &moves).unwrap();
        assert_eq!(mv.to_string(), "32-28");
        assert_eq!(input.from(), None);
    }

    #[test]
    fn clicks_select_switch_and_clear() {
        let moves = Position::initial().legal_moves();
        let mut input = MoveInput::default();
        // A piece without moves selects nothing.
        input.click(sq(47), &moves);
        assert_eq!(input.from(), None);
        input.click(sq(31), &moves);
        input.click(sq(34), &moves);
        assert_eq!(input.from(), sq(34));
        // Clicking the selected piece again deselects it.
        input.click(sq(34), &moves);
        assert_eq!(input.from(), None);
        input.click(sq(34), &moves);
        input.click(None, &moves);
        assert_eq!(input.from(), None);
    }

    #[test]
    fn ambiguous_captures_need_a_captured_piece() {
        // The king on 9 can reach 25 taking 13, 30 and either 33 or 42.
        let moves = Position::from_fen("W:WK9:B3,13,30,33,42")
            .unwrap()
            .legal_moves();
        let mut input = MoveInput::default();
        input.click(sq(9), &moves);
        assert_eq!(input.click(sq(25), &moves), None);
        assert!(input.is_ambiguous());
        // A piece both routes take does not decide.
        assert_eq!(input.click(sq(13), &moves), None);
        let mv = input.click(sq(42), &moves).unwrap();
        assert_eq!(mv.to_hub(), "9x25x13x30x42");
        assert!(!input.is_ambiguous());
    }

    #[test]
    fn captured_piece_can_be_chosen_first() {
        let moves = Position::from_fen("W:WK9:B3,13,30,33,42")
            .unwrap()
            .legal_moves();
        let mut input = MoveInput::default();
        input.click(sq(9), &moves);
        assert_eq!(input.click(sq(33), &moves), None);
        assert_eq!(input.via(), sq(33).unwrap().bitboard());
        let mv = input.click(sq(25), &moves).unwrap();
        assert_eq!(mv.to_hub(), "9x25x13x30x33");
    }
}
