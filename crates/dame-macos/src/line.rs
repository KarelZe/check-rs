//! The engine's expected line and the fast-forward preview along it,
//! independent of the UI toolkit.
//!
//! A [`Line`] is a variation from a fixed position. A [`Preview`] steps
//! through a line: it shows the position some plies ahead without touching
//! the game.

use dame::{Bitboard, Move, Position};

/// The moves the engine expects from `start`, all legal in turn.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Line {
    start: Position,
    moves: Vec<Move>,
}

impl Line {
    /// A line without moves, e.g. before a search has reported one.
    pub(crate) fn empty(start: Position) -> Self {
        Self {
            start,
            moves: Vec::new(),
        }
    }

    /// Reads a variation in Hub notation (`28x19x23`), stopping at the
    /// first move that does not parse.
    pub(crate) fn parse(start: Position, hub: &str) -> Self {
        let mut position = start;
        let mut moves = Vec::new();
        for text in hub.split_whitespace() {
            let Ok(mv) = position.parse_move(text) else {
                break;
            };
            moves.push(mv);
            position.play(mv);
        }
        Self { start, moves }
    }

    pub(crate) fn start(&self) -> Position {
        self.start
    }

    pub(crate) fn moves(&self) -> &[Move] {
        &self.moves
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    /// The position after the first `plies` moves (all of them if fewer).
    pub(crate) fn position_after(&self, plies: usize) -> Position {
        let mut position = self.start;
        for &mv in self.moves.iter().take(plies) {
            position.play(mv);
        }
        position
    }

    /// The rest of the line from `position`, if the line passes through it
    /// and goes on from there: the whole line from its start, the moves
    /// after the expected ones were played, and `None` once the game left
    /// the line or reached its end.
    pub(crate) fn from(&self, position: &Position) -> Option<Self> {
        let mut current = self.start;
        for (ply, &mv) in self.moves.iter().enumerate() {
            if current == *position {
                return Some(Self {
                    start: current,
                    moves: self.moves[ply..].to_vec(),
                });
            }
            current.play(mv);
        }
        None
    }

    /// The moves in short PDN (`28x19`), separated by spaces.
    pub(crate) fn pdn(moves: &[Move]) -> String {
        moves
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Stepping through a line on the board while the game stays as it is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Preview {
    line: Line,
    /// Moves of the line shown as played, at most all of them.
    ply: usize,
}

impl Preview {
    /// Starts one ply into `line`, or at its start while it is empty.
    pub(crate) fn new(line: Line) -> Self {
        let ply = usize::from(!line.is_empty());
        Self { line, ply }
    }

    pub(crate) fn line(&self) -> &Line {
        &self.line
    }

    pub(crate) fn ply(&self) -> usize {
        self.ply
    }

    /// Whether the line is still unknown, e.g. while the engine looks for it.
    pub(crate) fn is_waiting(&self) -> bool {
        self.line.is_empty()
    }

    /// Fills a waiting preview with the line found for its position and
    /// steps into it. Returns whether the preview took the line.
    pub(crate) fn fill(&mut self, line: Line) -> bool {
        if !self.is_waiting() || line.start() != self.line.start() {
            return false;
        }
        *self = Self::new(line);
        true
    }

    pub(crate) fn can_forward(&self) -> bool {
        self.ply < self.line.moves().len()
    }

    /// One ply further along the line, stopping at its end.
    pub(crate) fn forward(&mut self) {
        if self.can_forward() {
            self.ply += 1;
        }
    }

    /// One ply back; returns `false` when that is the game's own position,
    /// which ends the preview.
    #[must_use]
    pub(crate) fn back(&mut self) -> bool {
        self.ply = self.ply.saturating_sub(1);
        self.ply > 0
    }

    /// The position shown.
    pub(crate) fn position(&self) -> Position {
        self.line.position_after(self.ply)
    }

    /// The moves shown as played, oldest first.
    pub(crate) fn played(&self) -> &[Move] {
        &self.line.moves()[..self.ply]
    }

    /// The moves still ahead.
    pub(crate) fn ahead(&self) -> &[Move] {
        &self.line.moves()[self.ply..]
    }

    /// The pieces the last shown move took, gone from [`Self::position`].
    pub(crate) fn captured(&self) -> Bitboard {
        self.played()
            .last()
            .map_or(Bitboard::EMPTY, |mv| mv.captured())
    }
}

#[cfg(test)]
mod tests {
    use dame::Square;

    use super::*;

    const LINE: &str = "32-28 19-23 28x19x23 14x23x19";

    fn pdn(moves: &[Move]) -> String {
        Line::pdn(moves)
    }

    #[test]
    fn lines_parse_to_legal_moves() {
        let line = Line::parse(Position::initial(), LINE);
        assert_eq!(pdn(line.moves()), "32-28 19-23 28x19 14x23");
        assert_eq!(line.start(), Position::initial());
        // An illegal move ends the line.
        let line = Line::parse(Position::initial(), "32-28 99-98 19-23");
        assert_eq!(pdn(line.moves()), "32-28");
        assert!(Line::parse(Position::initial(), "").is_empty());
        assert_eq!(Line::empty(Position::initial()).moves().len(), 0);
    }

    #[test]
    fn positions_along_a_line() {
        let line = Line::parse(Position::initial(), LINE);
        assert_eq!(line.position_after(0), Position::initial());
        let mut expected = Position::initial();
        for (ply, &mv) in line.moves().iter().enumerate() {
            expected.play(mv);
            assert_eq!(line.position_after(ply + 1), expected);
        }
        // Past the end it stays at the last position.
        assert_eq!(line.position_after(99), expected);
    }

    #[test]
    fn the_rest_of_a_line_follows_the_game() {
        let line = Line::parse(Position::initial(), LINE);
        assert_eq!(line.from(&Position::initial()), Some(line.clone()));
        // After the first two expected moves, two are left.
        let rest = line.from(&line.position_after(2)).unwrap();
        assert_eq!(rest.start(), line.position_after(2));
        assert_eq!(pdn(rest.moves()), "28x19 14x23");
        // At the end of the line, or off it, there is nothing left.
        assert_eq!(line.from(&line.position_after(4)), None);
        let off = Position::initial().after(Position::initial().legal_moves()[0]);
        assert_ne!(off, line.position_after(1));
        assert_eq!(line.from(&off), None);
        assert_eq!(
            Line::empty(Position::initial()).from(&Position::initial()),
            None
        );
    }

    #[test]
    fn preview_steps_and_clamps() {
        let line = Line::parse(Position::initial(), LINE);
        let mut preview = Preview::new(line.clone());
        assert_eq!(preview.ply(), 1);
        assert!(!preview.is_waiting());
        assert_eq!(preview.position(), line.position_after(1));
        assert_eq!(pdn(preview.played()), "32-28");
        assert_eq!(pdn(preview.ahead()), "19-23 28x19 14x23");
        for _ in 0..10 {
            preview.forward();
        }
        assert_eq!(preview.ply(), 4);
        assert!(!preview.can_forward());
        assert_eq!(preview.position(), line.position_after(4));
        assert_eq!(pdn(preview.ahead()), "");
        assert!(preview.back());
        assert_eq!(preview.ply(), 3);
        assert!(preview.can_forward());
        assert!(preview.back());
        assert!(preview.back());
        // Back at the game's position, the preview ends.
        assert!(!preview.back());
        assert_eq!(preview.ply(), 0);
        assert!(!preview.back());
    }

    #[test]
    fn preview_marks_captured_pieces() {
        let line = Line::parse(Position::initial(), LINE);
        let mut preview = Preview::new(line);
        assert_eq!(preview.captured(), Bitboard::EMPTY);
        preview.forward();
        preview.forward();
        // 28x19 took the man on 23.
        let taken = Square::new(23).unwrap();
        assert_eq!(preview.captured(), taken.bitboard());
        assert_eq!(preview.position().piece_at(taken), None);
    }

    #[test]
    fn waiting_preview_takes_its_line_only() {
        let start = Position::initial();
        let mut preview = Preview::new(Line::empty(start));
        assert!(preview.is_waiting());
        assert_eq!(preview.ply(), 0);
        assert!(!preview.can_forward());
        preview.forward();
        assert_eq!(preview.ply(), 0);
        assert_eq!(preview.position(), start);
        // A line for another position does not fit.
        let other = Line::parse(start.after(start.legal_moves()[0]), "19-23");
        assert!(!preview.fill(other));
        assert!(preview.is_waiting());
        let line = Line::parse(start, LINE);
        assert!(preview.fill(line.clone()));
        assert_eq!(preview.ply(), 1);
        assert_eq!(preview.line(), &line);
        // A filled preview keeps its line.
        assert!(!preview.fill(Line::parse(start, "31-27")));
        assert_eq!(preview.line(), &line);
    }
}
