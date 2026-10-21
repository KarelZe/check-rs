//! Games: a start position, the moves played and the end-of-game rules.

use std::fmt;

use crate::moves::{Move, MoveList};
use crate::notation::ParseMoveError;
use crate::piece::Color;
use crate::position::{Position, PositionError};

/// Why a game ended.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum Termination {
    /// The side to move has no legal move (no pieces, or all blocked) and loses.
    NoMoves,
    /// The same position occurred for the third time with the same side to move.
    Repetition,
    /// 25 moves by each side with only kings moved and nothing captured.
    TwentyFiveMoves,
    /// A lone king against three pieces including at least one king, for 16
    /// moves by each side.
    SixteenMoves,
    /// A lone king against at most two pieces including at least one king,
    /// for 5 moves by each side.
    FiveMoves,
}

/// The result of a finished game.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct Outcome {
    /// The winner, or `None` for a draw.
    pub winner: Option<Color>,
    /// The rule that ended the game.
    pub termination: Termination,
}

impl Outcome {
    /// The PDN result string: `2-0`, `0-2` or `1-1`.
    pub const fn result(self) -> &'static str {
        match self.winner {
            Some(Color::White) => "2-0",
            Some(Color::Black) => "0-2",
            None => "1-1",
        }
    }
}

/// The error returned when a move cannot be played.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum IllegalMoveError {
    /// The move is not legal in the current position.
    Illegal(Move),
    /// The move text could not be matched to a legal move.
    Parse(ParseMoveError),
}

impl fmt::Display for IllegalMoveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Illegal(mv) => write!(f, "illegal move {mv}"),
            Self::Parse(err) => err.fmt(f),
        }
    }
}

impl std::error::Error for IllegalMoveError {}

impl From<ParseMoveError> for IllegalMoveError {
    fn from(err: ParseMoveError) -> Self {
        Self::Parse(err)
    }
}

/// Per-ply bookkeeping for the draw rules.
#[derive(Copy, Clone, Debug)]
struct Counters {
    /// Plies since the last man move or capture.
    reversible: u32,
    /// Plies since the last capture or promotion changed the material.
    material: u32,
}

/// A game in progress: a start position plus the moves played from it.
///
/// `Game` enforces legality and implements the FMJD end-of-game rules:
/// no legal moves, threefold repetition, the 25-move rule and the 16- and
/// 5-move rules for king endgames.
#[derive(Clone, Debug)]
pub struct Game {
    positions: Vec<Position>,
    moves: Vec<Move>,
    counters: Vec<Counters>,
}

impl Game {
    /// A new game from the standard starting position.
    pub fn new() -> Self {
        Self::from_position(Position::initial())
    }

    /// A new game starting from `position`.
    pub fn from_position(position: Position) -> Self {
        Self {
            positions: vec![position],
            moves: Vec::new(),
            counters: vec![Counters {
                reversible: 0,
                material: 0,
            }],
        }
    }

    /// A new game starting from a FEN position.
    pub fn from_fen(fen: &str) -> Result<Self, PositionError> {
        Position::from_fen(fen).map(Self::from_position)
    }

    /// The current position.
    #[inline]
    pub fn position(&self) -> &Position {
        self.positions
            .last()
            .expect("a game has at least one position")
    }

    /// The start position.
    pub fn start(&self) -> &Position {
        &self.positions[0]
    }

    /// The moves played so far.
    pub fn moves(&self) -> &[Move] {
        &self.moves
    }

    /// Every position of the game, from the start to the current one.
    pub fn positions(&self) -> &[Position] {
        &self.positions
    }

    /// The legal moves in the current position; empty once the game is over.
    pub fn legal_moves(&self) -> MoveList {
        if self.outcome().is_some() {
            MoveList::new()
        } else {
            self.position().legal_moves()
        }
    }

    /// Plays `mv` after checking that it is legal.
    pub fn play(&mut self, mv: Move) -> Result<(), IllegalMoveError> {
        if !self.legal_moves().contains(&mv) {
            return Err(IllegalMoveError::Illegal(mv));
        }
        self.play_unchecked(mv);
        Ok(())
    }

    /// Parses a move in PDN or Hub notation (`32-28`, `28x19`, `28x19x23`)
    /// and plays it.
    pub fn play_str(&mut self, text: &str) -> Result<Move, IllegalMoveError> {
        let mv = self.position().parse_move(text)?;
        self.play(mv)?;
        Ok(mv)
    }

    /// Plays `mv` without checking legality.
    pub fn play_unchecked(&mut self, mv: Move) {
        let pos = *self.position();
        let next = pos.after(mv);
        let us = pos.side_to_move();
        let man_moved = pos.men(us).contains(mv.from());
        let promoted = man_moved && next.kings(us).contains(mv.to());
        let last = *self.counters.last().expect("counters match positions");
        self.counters.push(Counters {
            reversible: if man_moved || mv.is_capture() {
                0
            } else {
                last.reversible + 1
            },
            material: if promoted || mv.is_capture() {
                0
            } else {
                last.material + 1
            },
        });
        self.positions.push(next);
        self.moves.push(mv);
    }

    /// Takes back the last move.
    pub fn undo(&mut self) -> Option<Move> {
        let mv = self.moves.pop()?;
        self.positions.pop();
        self.counters.pop();
        Some(mv)
    }

    /// Plies since the last man move or capture.
    pub fn reversible_plies(&self) -> u32 {
        self.counters
            .last()
            .expect("counters match positions")
            .reversible
    }

    /// How often the current position has occurred, including now.
    pub fn repetitions(&self) -> usize {
        let current = self.position().hash();
        let window = self.reversible_plies() as usize;
        let n = self.positions.len();
        self.positions[n - 1 - window..]
            .iter()
            .rev()
            .step_by(2)
            .filter(|pos| pos.hash() == current)
            .count()
    }

    /// The outcome if the game is over.
    pub fn outcome(&self) -> Option<Outcome> {
        let pos = self.position();
        let us = pos.side_to_move();
        if pos.legal_moves().is_empty() {
            return Some(Outcome {
                winner: Some(!us),
                termination: Termination::NoMoves,
            });
        }
        let draw = |termination| {
            Some(Outcome {
                winner: None,
                termination,
            })
        };
        if self.repetitions() >= 3 {
            return draw(Termination::Repetition);
        }
        let counters = *self.counters.last().expect("counters match positions");
        if counters.reversible >= 50 {
            return draw(Termination::TwentyFiveMoves);
        }
        let lone_king = |color: Color| {
            let pieces = pos.pieces(color);
            pieces.count() == 1 && pieces == pos.kings(color)
        };
        for color in [Color::White, Color::Black] {
            let other = pos.pieces(!color).count();
            if !lone_king(color) || pos.kings(!color).is_empty() {
                continue;
            }
            if other == 3 && counters.material >= 32 {
                return draw(Termination::SixteenMoves);
            }
            if other <= 2 && counters.material >= 10 {
                return draw(Termination::FiveMoves);
            }
        }
        None
    }

    /// Whether the game is over.
    pub fn is_over(&self) -> bool {
        self.outcome().is_some()
    }
}

impl Default for Game {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn play_and_undo() {
        let mut game = Game::new();
        game.play_str("32-28").unwrap();
        game.play_str("19-23").unwrap();
        let capture = game.play_str("28x19").unwrap();
        assert!(capture.is_capture());
        assert_eq!(game.moves().len(), 3);
        assert!(game.play_str("32-28").is_err());
        assert_eq!(game.undo(), Some(capture));
        assert_eq!(game.position().side_to_move(), Color::White);
    }

    #[test]
    fn no_moves_loses() {
        let game = Game::from_fen("W:W46:B41,37").unwrap();
        let outcome = game.outcome().unwrap();
        assert_eq!(outcome.winner, Some(Color::Black));
        assert_eq!(outcome.termination, Termination::NoMoves);
        assert_eq!(outcome.result(), "0-2");
    }

    /// Plays quiet moves that never repeat a position or allow a capture
    /// until the game ends, and returns the number of plies played.
    fn shuffle(game: &mut Game) -> usize {
        let mut seen = std::collections::HashSet::new();
        seen.insert(game.position().hash());
        let mut plies = 0;
        while !game.is_over() {
            let pos = *game.position();
            let mv = pos
                .legal_moves()
                .iter()
                .copied()
                .find(|&mv| {
                    let next = pos.after(mv);
                    !mv.is_capture() && !next.has_captures() && !seen.contains(&next.hash())
                })
                .expect("a quiet, new move exists");
            game.play(mv).unwrap();
            seen.insert(game.position().hash());
            plies += 1;
        }
        plies
    }

    #[test]
    fn threefold_repetition() {
        // Kings in opposite corners, far from each other's diagonals.
        let mut game = Game::from_fen("W:WK50:BK1").unwrap();
        for _ in 0..2 {
            for mv in ["50-44", "1-7", "44-50", "7-1"] {
                assert!(game.outcome().is_none());
                game.play_str(mv).unwrap();
            }
        }
        assert_eq!(game.repetitions(), 3);
        assert_eq!(game.outcome().unwrap().termination, Termination::Repetition);
    }

    #[test]
    fn reversible_plies_reset_on_man_moves() {
        let mut game = Game::from_fen("W:WK50,31:BK1,20").unwrap();
        game.play_str("50-44").unwrap();
        game.play_str("1-7").unwrap();
        assert_eq!(game.reversible_plies(), 2);
        game.play_str("31-26").unwrap();
        assert_eq!(game.reversible_plies(), 0);
    }

    #[test]
    fn twenty_five_move_rule() {
        let mut game = Game::from_fen("W:WK50,K46:BK1,K5").unwrap();
        assert_eq!(shuffle(&mut game), 50);
        assert_eq!(
            game.outcome().unwrap().termination,
            Termination::TwentyFiveMoves
        );
    }

    #[test]
    fn sixteen_move_rule() {
        let mut game = Game::from_fen("W:WK50:BK1,K5,K3").unwrap();
        assert_eq!(shuffle(&mut game), 32);
        assert_eq!(
            game.outcome().unwrap().termination,
            Termination::SixteenMoves
        );
    }

    #[test]
    fn five_move_rule() {
        let mut game = Game::from_fen("W:WK50:BK1,K5").unwrap();
        assert_eq!(shuffle(&mut game), 10);
        let outcome = game.outcome().unwrap();
        assert_eq!(
            (outcome.winner, outcome.termination),
            (None, Termination::FiveMoves)
        );
    }
}
