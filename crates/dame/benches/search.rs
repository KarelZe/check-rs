//! Fixed-depth search benchmarks.
//!
//! Run with `cargo bench -p dame --bench search`. Each iteration starts
//! from a fresh searcher, so the transposition table starts empty.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dame::{Game, Limits, Searcher};

const POSITIONS: [(&str, &str, u32); 2] = [
    ("initial", "W:W31-50:B1-20", 8),
    (
        "woldouby",
        "W:B12,13,14,16,18,19,21,23,24,26:W25,27,28,30,32,33,34,35,37,38",
        10,
    ),
];

fn bench_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("search");
    group.sample_size(10);
    for (name, fen, depth) in POSITIONS {
        let game = Game::from_fen(fen).unwrap();
        let limits = Limits::depth(depth);
        group.bench_function(name, |b| {
            b.iter(|| {
                let mut searcher = Searcher::default();
                searcher.search(black_box(&game), &limits, |_| {})
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_search);
criterion_main!(benches);
