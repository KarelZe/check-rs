//! Static evaluation.
//!
//! Scores are in centi-men from the point of view of the side to move: a man
//! is worth 100. The search is generic over [`Evaluate`], so a pattern-based
//! or neural evaluation can replace [`Classical`] without touching the
//! search.

use crate::bitboard::Bitboard;
use crate::piece::Color;
use crate::position::Position;

/// A static evaluation function.
pub trait Evaluate: Send + Sync {
    /// The score of `pos` from the point of view of the side to move, in
    /// centi-men. Must stay well inside `±`[`crate::search::MAX_EVAL`].
    fn evaluate(&self, pos: &Position) -> i32;
}

/// The squares of row `row` (0 is the top row, squares 1..=5).
const fn row_mask(row: u32) -> Bitboard {
    let mut bits = 0;
    let mut i = 0;
    while i < 5 {
        let d = 5 * row + i;
        bits |= 1 << (d + d / 10);
        i += 1;
    }
    Bitboard(bits)
}

const ROWS: [Bitboard; 10] = {
    let mut rows = [Bitboard::EMPTY; 10];
    let mut row = 0;
    while row < 10 {
        rows[row] = row_mask(row as u32);
        row += 1;
    }
    rows
};

/// Squares 22..=24 and 27..=29.
const CENTER: Bitboard = Bitboard(0b111 << 23 | 0b111 << 28);

/// The squares a side keeps on its back row to stop enemy men from crowning:
/// 47..=49 for White, 2..=4 for Black.
const WHITE_GUARD: Bitboard = Bitboard(0b111 << 50);
const BLACK_GUARD: Bitboard = Bitboard(0b111 << 1);

/// A small hand-written evaluation: material, development, center control
/// and back-row guards, scaled down in materially drawish endgames.
///
/// It is deliberately simple and serves as the baseline that tuned or
/// learned evaluations have to beat.
#[derive(Clone, Debug)]
pub struct Classical {
    /// Value of a man.
    pub man: i32,
    /// Value of a king.
    pub king: i32,
    /// Bonus per row a man has advanced.
    pub advance: i32,
    /// Bonus per man on a center square.
    pub center: i32,
    /// Bonus per man guarding the back row while the opponent has no king.
    pub guard: i32,
}

impl Default for Classical {
    fn default() -> Self {
        Self {
            man: 100,
            king: 300,
            advance: 2,
            center: 6,
            guard: 5,
        }
    }
}

impl Classical {
    /// The score from White's point of view.
    fn white_score(&self, pos: &Position) -> i32 {
        let (wm, bm) = (pos.men(Color::White), pos.men(Color::Black));
        let (wk, bk) = (pos.kings(Color::White), pos.kings(Color::Black));

        let mut score = self.man * (wm.count() as i32 - bm.count() as i32)
            + self.king * (wk.count() as i32 - bk.count() as i32);

        // White men advance towards row 0, Black men towards row 9.
        let mut advance = 0;
        for (row, &mask) in ROWS.iter().enumerate() {
            let row = row as i32;
            advance += (9 - row) * (wm & mask).count() as i32 - row * (bm & mask).count() as i32;
        }
        score += self.advance * advance;

        score += self.center * ((wm & CENTER).count() as i32 - (bm & CENTER).count() as i32);

        if bk.is_empty() {
            score += self.guard * (wm & WHITE_GUARD).count() as i32;
        }
        if wk.is_empty() {
            score -= self.guard * (bm & BLACK_GUARD).count() as i32;
        }

        // Few pieces against a king rarely win: damp the advantage.
        let (white, black) = (pos.pieces(Color::White), pos.pieces(Color::Black));
        if (score > 0 && white.count() <= 3 && !bk.is_empty())
            || (score < 0 && black.count() <= 3 && !wk.is_empty())
        {
            score /= 8;
        }
        score
    }
}

impl Evaluate for Classical {
    fn evaluate(&self, pos: &Position) -> i32 {
        let score = self.white_score(pos);
        match pos.side_to_move() {
            Color::White => score,
            Color::Black => -score,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::square::Square;

    #[test]
    fn masks() {
        let numbers = |bb: Bitboard| bb.iter().map(Square::number).collect::<Vec<_>>();
        assert_eq!(numbers(ROWS[0]), [1, 2, 3, 4, 5]);
        assert_eq!(numbers(ROWS[9]), [46, 47, 48, 49, 50]);
        assert_eq!(numbers(CENTER), [22, 23, 24, 27, 28, 29]);
        assert_eq!(numbers(WHITE_GUARD), [47, 48, 49]);
        assert_eq!(numbers(BLACK_GUARD), [2, 3, 4]);
    }

    #[test]
    fn initial_position_is_balanced() {
        assert_eq!(Classical::default().evaluate(&Position::initial()), 0);
    }

    #[test]
    fn symmetric_under_color_flip() {
        let eval = Classical::default();
        for fen in [
            "W:W28,K40,45:B19,23,K32",
            "B:W18,24,27,28,K10,K15:B12,16,20,K22,K25,K29",
            "W:B12,13,14,16,18,19,21,23,24,26:W25,27,28,30,32,33,34,35,37,38",
            "W:W31,32,33:BK1",
        ] {
            let pos = Position::from_fen(fen).unwrap();
            assert_eq!(eval.evaluate(&pos), eval.evaluate(&pos.flipped()), "{fen}");
        }
    }

    #[test]
    fn material_counts() {
        let eval = Classical::default();
        let up_a_man = Position::from_fen("W:W31-50:B2-20").unwrap();
        assert!(eval.evaluate(&up_a_man) > 50);
        assert!(eval.evaluate(&Position::from_fen("B:W31-50:B2-20").unwrap()) < -50);
    }
}
