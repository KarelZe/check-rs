//! Move generation benchmarks.
//!
//! Run with `cargo bench -p dame --bench movegen`.

use std::hint::black_box;

use criterion::{Criterion, criterion_group, criterion_main};
use dame::{Position, perft};

const POSITIONS: [(&str, &str, u32); 3] = [
    ("initial", "W:W31-50:B1-20", 6),
    (
        "woldouby",
        "W:B12,13,14,16,18,19,21,23,24,26:W25,27,28,30,32,33,34,35,37,38",
        9,
    ),
    (
        "kings",
        "B:BK17,K24:W6,9,10,11,20,21,22,23,30,K31,33,37,41,42,43,44,46",
        4,
    ),
];

fn bench_perft(c: &mut Criterion) {
    let mut group = c.benchmark_group("perft");
    for (name, fen, depth) in POSITIONS {
        let pos = Position::from_fen(fen).unwrap();
        group.bench_function(name, |b| {
            b.iter(|| perft(black_box(&pos), black_box(depth)));
        });
    }
    group.finish();
}

fn bench_legal_moves(c: &mut Criterion) {
    let pos = Position::from_fen(POSITIONS[2].1).unwrap();
    c.bench_function("legal_moves/kings", |b| {
        b.iter(|| black_box(&pos).legal_moves().len());
    });
}

criterion_group!(benches, bench_perft, bench_legal_moves);
criterion_main!(benches);
