//! Opening book loading benchmarks.
//!
//! Run with `cargo bench -p dame --bench book`. The PDN collection is
//! generated here from pseudo-random legal moves with a fixed seed, so the
//! input is deterministic and needs no game files.

use std::fmt::Write;
use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dame::{Game, OpeningBook};

const GAMES: usize = 4000;
const MIN_PLIES: u64 = 40;
const MAX_PLIES: u64 = 80;

/// A xorshift64 generator: tiny, deterministic and good enough to pick moves.
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

/// `GAMES` games of random legal moves from the start, formatted as PDN with
/// tags, move numbers and results.
fn collection() -> String {
    let mut rng = XorShift(0x9E37_79B9_7F4A_7C15);
    let mut pdn = String::new();
    for index in 1..=GAMES {
        let plies = MIN_PLIES + rng.next() % (MAX_PLIES - MIN_PLIES + 1);
        let mut game = Game::new();
        let mut moves = String::new();
        for ply in 0..plies {
            if game.outcome().is_some() {
                break;
            }
            let legal = game.legal_moves();
            let mv = legal[(rng.next() % legal.len() as u64) as usize];
            // Short notation unless it is ambiguous, as in real collections.
            let short = mv.to_string();
            let text = if game.position().parse_move(&short) == Ok(mv) {
                short
            } else {
                mv.to_hub()
            };
            if ply % 2 == 0 {
                write!(moves, "{}. ", ply / 2 + 1).unwrap();
            }
            moves.push_str(&text);
            moves.push(if ply % 10 == 9 { '\n' } else { ' ' });
            game.play_unchecked(mv);
        }
        let result = game.outcome().map_or("*", |outcome| outcome.result());
        writeln!(pdn, "[Event \"Synthetic {index}\"]").unwrap();
        writeln!(pdn, "[White \"Random\"]\n[Black \"Random\"]").unwrap();
        writeln!(pdn, "[Result \"{result}\"]\n[GameType \"20\"]\n").unwrap();
        writeln!(pdn, "{moves}{result}\n").unwrap();
    }
    pdn
}

fn bench_from_pdn(c: &mut Criterion) {
    let pdn = collection();
    let mut group = c.benchmark_group("book");
    group.sample_size(20);
    group.bench_function("from_pdn", |b| {
        b.iter(|| OpeningBook::from_pdn(black_box(&pdn), OpeningBook::DEFAULT_PLIES).unwrap());
    });
    group.finish();
}

criterion_group!(benches, bench_from_pdn);
criterion_main!(benches);
